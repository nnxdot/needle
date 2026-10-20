import SwiftUI
import UniformTypeIdentifiers

/// Settings, by section, as on the desktop and Android.
struct SettingsView: View {
    @Environment(Store.self) private var store
    /// This build has Discord's library.
    @State private var discord = false

    var body: some View {
        List {
            Section {
                row("Library", "Music folders and online lookups", "music.note.house", .orange, .settingsLibrary)
                row("Playback", "Crossfade and even loudness", "play.circle", .blue, .settingsPlayback)
                row("Sound", "Equalizer, balance, and headphones", "slider.vertical.3", .purple, .settingsSound)
                row("Appearance", "Themes, colours, and the player", "paintpalette", .pink, .settingsAppearance)
                row("Scrobbling", "Last.fm and ListenBrainz", "waveform", .red, .settingsScrobbling)
                if discord {
                    row("Discord", "Show what you play on your profile", "bubble.left.and.text.bubble.right", .indigo, .settingsDiscord)
                }
            }
            Section {
                row("Plugins", "Music servers, NetEase lyrics, effects", "puzzlepiece.extension", .teal, .plugins)
                row("Your computer", "Control Needle there, play its music here", "laptopcomputer", .indigo, .computer)
                row("Sync", "Move history, ratings, and playlists", "arrow.triangle.2.circlepath", .yellow, .sync)
                row("Fix my library", "Tags, duplicates, covers, and files", "wrench.and.screwdriver", .green, .fixLibrary)
                row("Import", "Other players and listening history", "square.and.arrow.down", .cyan, .importing)
            }
            Section {
                row("What's new", "In this version of Needle", "sparkles", .pink, .whatsNew)
                row("About", "Version, updates, and privacy", "info.circle", .gray, .about)
            }
        }
        .navigationTitle("Settings")
        .themed()
        .task { discord = await store.load { $0.discordAccount() }?.available ?? false }
    }

    private func row(_ title: String, _ detail: String, _ symbol: String, _ tint: Color, _ route: Route) -> some View {
        NavigationLink(value: route) {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.body.weight(.semibold)).foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(tint.gradient, in: .rect(cornerRadius: 8))
                VStack(alignment: .leading, spacing: 1) {
                    Text(title)
                    Text(detail).font(.caption).foregroundStyle(.secondary)
                }
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
                        Text(folder == store.musicFolder.path ? "Needle's folder" : (folder as NSString).lastPathComponent)
                        Text(folder == store.musicFolder.path ? "Files › On My iPhone › Needle" : folder)
                            .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    .deleteDisabled(folder == store.musicFolder.path)
                }
                .onDelete { rows in
                    for folder in rows.map({ folders[$0] }) { store.run { try $0.removeFolder(path: folder) } }
                    folders.remove(atOffsets: rows)
                    store.libraryChanged()
                }
                Button("Read the music again") { store.rescan() }.disabled(store.scan.running)
            } header: {
                Text("Music folders")
            } footer: {
                if store.scan.running { Text("Reading… \(count(Int(store.scan.imported), "song")) so far") }
            }
            if let settings {
                let save = saved($settings, store) { try $0.setPlaybackSettings(value: $1) }
                Section {
                    Toggle("Look up online", isOn: Binding(get: { settings.onlineMedia }, set: { var s = settings; s.onlineMedia = $0; save(s) }))
                } footer: {
                    Text("Lyrics from LRCLIB when a song has none, and covers and artist photos from MusicBrainz and Wikidata. Off: Needle asks none of these. Your own music server is still asked for its songs' lyrics.")
                }
                Section {
                    if hasFanart {
                        LabeledContent("fanart.tv key", value: "Set")
                        Button("Remove the Key", role: .destructive) {
                            store.run { try $0.setFanartKey(key: "") }
                            hasFanart = false
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
                } header: {
                    Text("Artists' logos")
                } footer: {
                    Text("With online lookups on and a free key from fanart.tv, artist pages show the artist's logo. Needle asks MusicBrainz for the artist's ID by name, and sends fanart.tv only that ID. The key stays in the iPhone Keychain.")
                }
                Section {
                    Toggle("Measure songs for radio", isOn: Binding(get: { settings.soundAnalysis }, set: { var s = settings; s.soundAnalysis = $0; save(s) }))
                    if measured.count == 2 {
                        LabeledContent("Measured", value: "\(measured[0].formatted()) of \(measured[1].formatted())")
                        Button("Measure now") { store.core.measureInBackground(); store.message = "Measuring in the background" }
                            .disabled(measured[0] >= measured[1])
                    }
                } footer: {
                    Text("Radio picks songs that sound like each other. Needle measures a few songs at a time.")
                }
                Section {
                    TextField("For example: rating is at least 4", text: Binding(get: { settings.autoplayQuery }, set: { var s = settings; s.autoplayQuery = $0; save(s) }))
                        .textInputAutocapitalization(.never)
                } header: {
                    Text("When nothing is up next")
                } footer: {
                    Text("Needle keeps playing songs that match this search. Empty: the music stops.")
                }
            }
            Section {
                Button("Back up the library") {
                    let path = FileManager.default.temporaryDirectory.appendingPathComponent("Needle library.needlebackup").path
                    Task {
                        if await store.ask({ try $0.backupLibrary(path: path) }) != nil {
                            backup = URL(fileURLWithPath: path)
                            moving = true
                        }
                    }
                }
            } footer: {
                Text("One file with your songs' plays, stars, playlists, and settings, but no music.")
            }
        }
        .navigationTitle("Library")
        .themed()
        .fileMover(isPresented: $moving, file: backup) { _ in }
        .task {
            folders = await store.load { try $0.folders() } ?? []
            settings = await store.load { try $0.playbackSettings() }
            hasFanart = await store.load { $0.hasFanartKey() } ?? false
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
                Section {
                    VStack(alignment: .leading) {
                        LabeledContent("Crossfade", value: settings.crossfade < 0.5 ? "Off" : "\(Int(settings.crossfade.rounded())) s")
                        Slider(value: Binding(get: { Double(settings.crossfade) }, set: { var s = settings; s.crossfade = Float($0.rounded()); save(s) }), in: 0...12, step: 1)
                    }
                } footer: {
                    Text("Songs overlap as one ends and the next begins. Albums that play on without a gap stay without one.")
                }
                Section {
                    Picker("Even loudness", selection: Binding(
                        get: { !settings.replayGain ? 0 : (settings.albumGain ? 2 : 1) },
                        set: { var s = settings; s.replayGain = $0 > 0; s.albumGain = $0 == 2; save(s) })) {
                        Text("Off").tag(0)
                        Text("Song").tag(1)
                        Text("Album").tag(2)
                    }
                    .pickerStyle(.segmented)
                } footer: {
                    Text("Uses the songs' ReplayGain tags. Album keeps an album's own quiet and loud songs as they are.")
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
                    Toggle("Mono", isOn: Binding(get: { sound.mono }, set: { var s = sound; s.mono = $0; save(s) }))
                    Toggle(isOn: Binding(get: { sound.crossfeed }, set: { var s = sound; s.crossfeed = $0; save(s) })) {
                        Text("Headphone crossfeed")
                        Text("Mixes a little of each side into the other, as speakers do, for less tiring listening.")
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

// MARK: - Appearance

struct AppearanceView: View {
    @Environment(Store.self) private var store
    @State private var addingTheme = false

    var body: some View {
        let app = store.app
        Form {
            Section("Theme") {
                Picker("Theme", selection: Binding(get: { app.theme }, set: { t in store.updateApp { $0.theme = t } })) {
                    Text("Night").tag("night")
                    Text("Midnight").tag("midnight")
                    Text("Day").tag("day")
                    ForEach(store.themes, id: \.id) { Text($0.name).tag($0.id) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
                ForEach(store.themes.filter { !$0.readOnly }, id: \.id) { theme in
                    NavigationLink(value: Route.themeEditor(theme.id)) { Label("Edit \(theme.name)", systemImage: "pencil") }
                }
                NavigationLink(value: Route.themeEditor(nil)) { Label("New theme", systemImage: "plus") }
                Button("Add a theme file…", systemImage: "doc.badge.plus") { addingTheme = true }
            }
            Section {
                ColorPicker("Accent", selection: Binding(
                    get: { Color(hex: app.accentColor) ?? ThemeColors.pink },
                    set: { c in store.updateApp { $0.accentColor = c.hex } }), supportsOpacity: false)
                if !app.accentColor.isEmpty {
                    Button("Use the theme's accent") { store.updateApp { $0.accentColor = "" } }
                }
                toggle("Colours from the song", "The whole app takes the colours of the song playing.", \.ambientColors)
                toggle("Colours from covers", "Album, playlist, and artist pages take their cover's colours.", \.coverColors)
            } header: {
                Text("Colours")
            }
            Section("Player") {
                toggle("Immersive cover", "The cover fills the top of the player, edge to edge, as in Apple Music.", \.immersiveCover)
                toggle("Moving covers", "A video beside the songs, such as cover.mp4, plays as the cover.", \.animatedCovers)
                toggle("Cover behind the player", app.immersiveCover ? "Behind the lyrics and the queue, the playing cover, blurred." : "The player's background is the playing cover, blurred.", \.coverBackdrop)
                if app.coverBackdrop {
                    toggle("Moving cover", nil, \.movingBackdrop)
                    VStack(alignment: .leading) {
                        LabeledContent("Softness", value: "\(Int(app.backdropBlur))")
                        Slider(value: Binding(get: { Double(app.backdropBlur) }, set: { v in store.updateApp { $0.backdropBlur = Float(v.rounded()) } }), in: 20...120)
                    }
                }
                // The immersive cover is always square, across the top.
                if !app.immersiveCover {
                    Picker("Cover", selection: Binding(get: { app.coverShape }, set: { v in store.updateApp { $0.coverShape = v } })) {
                        Text("Square").tag("square")
                        Text("Record").tag("round")
                    }
                }
                Picker("Beside play", selection: Binding(get: { app.sideButtons }, set: { v in store.updateApp { $0.sideButtons = v } })) {
                    Text("Skip songs").tag("skip")
                    Text("Jump 10 seconds").tag("jump")
                }
                toggle("Volume slider", nil, \.volumeSlider)
                toggle("Swipe the cover to skip", nil, \.swipeCover)
                toggle("Open the player on play", "The player opens by itself when you choose a song.", \.openPlayerOnPlay)
            }
            Section("Lyrics") {
                toggle("Live lyrics", "Lyrics move with the song, the line playing lit.", \.liveLyrics)
                toggle("Keep the screen on", "While the lyrics show.", \.keepScreenOnLyrics)
            }
            Section("Motion and touch") {
                toggle("Haptics", "A little tap on play, pause, and long presses.", \.haptics)
                toggle("Reduce motion", "Needle's own switch, besides the iPhone's.", \.reduceMotion)
            }
        }
        .navigationTitle("Appearance")
        .themed()
        .fileImporter(isPresented: $addingTheme, allowedContentTypes: [UTType(filenameExtension: "toml") ?? .plainText, .plainText]) { result in
            guard case .success(let url) = result, let path = store.copyIn(url) else { return }
            Task {
                if let id = await store.ask({ try $0.importTheme(path: path) }) {
                    store.reloadThemes()
                    store.updateApp { $0.theme = id }
                }
            }
        }
    }

    private func toggle(_ title: String, _ detail: String?, _ key: WritableKeyPath<AppSettings, Bool>) -> some View {
        Toggle(isOn: Binding(get: { store.app[keyPath: key] }, set: { v in store.updateApp { $0[keyPath: key] = v } })) {
            Text(title)
            if let detail { Text(detail) }
        }
    }
}

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

    var body: some View {
        Form {
            if let a = accounts {
                Section {
                    if let user = a.lastfmUser {
                        LabeledContent("Signed in", value: user.isEmpty ? "Yes" : user)
                        Toggle("Send listens", isOn: Binding(get: { a.lastfmOn }, set: { on in
                            store.run { try $0.setScrobbling(lastfm: on, listenbrainz: nil) }; Task { await reload() }
                        }))
                        Button("Sign out", role: .destructive) { store.run { try $0.signOut(service: "lastfm") }; Task { await reload() } }
                    } else if waitingForBrowser {
                        Text("Allow Needle on the Last.fm page, then come back.")
                        Button("I allowed Needle") {
                            Task {
                                if await store.ask({ try $0.lastfmComplete() }) != nil { waitingForBrowser = false }
                                await reload()
                            }
                        }
                    } else {
                        if !a.lastfmKeyBuiltIn {
                            TextField("API key", text: $apiKey).textInputAutocapitalization(.never).autocorrectionDisabled()
                            SecureField("Shared secret", text: $secret)
                        }
                        Button("Sign in to Last.fm") {
                            let (k, s) = (apiKey, secret)
                            Task {
                                if let page = await store.ask({ try $0.lastfmBegin(apiKey: k, secret: s) }), let url = URL(string: page) {
                                    waitingForBrowser = true
                                    openURL(url)
                                }
                            }
                        }
                        .disabled(!a.lastfmKeyBuiltIn && (apiKey.isEmpty || secret.isEmpty))
                    }
                } header: {
                    Text("Last.fm")
                } footer: {
                    if a.lastfmUser == nil && !a.lastfmKeyBuiltIn {
                        Text("This build has no Last.fm key: make one for free at last.fm/api/account/create.")
                    }
                }
                Section("ListenBrainz") {
                    if let user = a.listenbrainzUser {
                        LabeledContent("Signed in", value: user)
                        Toggle("Send listens", isOn: Binding(get: { a.listenbrainzOn }, set: { on in
                            store.run { try $0.setScrobbling(lastfm: nil, listenbrainz: on) }; Task { await reload() }
                        }))
                        Button("Sign out", role: .destructive) { store.run { try $0.signOut(service: "listenbrainz") }; Task { await reload() } }
                    } else {
                        SecureField("User token", text: $token)
                        Button("Sign in") {
                            let t = token
                            Task { _ = await store.ask { try $0.listenbrainzSignIn(token: t) }; await reload() }
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
                            Text("Artist names as Apple Music writes them")
                            Text("키키 is sent as KiiiKiii, for example.")
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
            await reload()
            settings = await store.load { try $0.playbackSettings() }
        }
    }

    private func reload() async {
        accounts = await store.load { $0.accounts() }
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
                    Text("Discord asks you to allow Needle once. Needle then sets your status itself, and keeps Discord's key in the iPhone Keychain. It sends Discord only what the card shows.")
                }
            }
            if let s = settings {
                let save = saved($settings, store) { try $0.setDiscordSettings(value: $1) }
                Section {
                    Toggle("Show what you're playing on Discord", isOn: Binding(get: { s.on }, set: { var n = s; n.on = $0; save(n) }))
                }
                if s.on {
                    Section("The card") {
                        choice("Listening to …", [("song", "Song"), ("artist", "Artist"), ("needle", "Needle")], s.title) { var n = s; n.title = $0; save(n) }
                        choice("First line", [("artist", "Artist"), ("song", "Song"), ("album", "Album")], s.top) { var n = s; n.top = $0; save(n) }
                        choice("Second line", [("song", "Song"), ("artist", "Artist"), ("album", "Album"), ("none", "None")], s.middle) { var n = s; n.middle = $0; save(n) }
                        choice("Third line", [("album", "Album"), ("song", "Song"), ("artist", "Artist"), ("none", "None")], s.bottom) { var n = s; n.bottom = $0; save(n) }
                    }
                    Section {
                        Toggle("Look up covers", isOn: Binding(get: { s.covers }, set: { var n = s; n.covers = $0; save(n) }))
                        Toggle(isOn: Binding(get: { s.logo }, set: { var n = s; n.logo = $0; save(n) })) {
                            Text("Show the Needle logo")
                            Text("A badge on the cover, and the picture when there is no cover")
                        }
                    } header: {
                        Text("Picture")
                    } footer: {
                        Text("Discord can only show pictures from the web, so Needle finds the cover in Apple's iTunes catalog by artist and song name. Your files never leave this iPhone. Without a picture, Discord hides the third line.")
                    }
                    Section {
                        Toggle(isOn: Binding(get: { s.paused }, set: { var n = s; n.paused = $0; save(n) })) {
                            Text("Show while paused")
                            Text("The song stays, marked Paused")
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
                    } footer: {
                        Text("After this long paused or stopped, the status goes away. It comes back when you play again.")
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
                Text("Plugins add music servers (Navidrome, Subsonic), lyrics from NetEase, sound effects, and commands. They run inside Needle, with only the permissions they list.")
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
        plugins = await store.load { $0.plugins() }
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
    private struct Release: Identifiable {
        let version: String
        var tagline: String?
        var lines: [(title: String, text: String)] = []
        var id: String { version }
    }

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
                if body.hasPrefix("["), let end = body.firstIndex(of: "]") { body = String(body[body.index(after: end)...]).trimmingCharacters(in: .whitespaces) }
                if line.hasPrefix("- ") || line.hasPrefix("! ") {
                    if body.hasPrefix("**"), let close = body.range(of: "**", range: body.index(body.startIndex, offsetBy: 2)..<body.endIndex) {
                        let title = String(body[body.index(body.startIndex, offsetBy: 2)..<close.lowerBound]).trimmingCharacters(in: CharacterSet(charactersIn: "."))
                        all[all.count - 1].lines.append((title, String(body[close.upperBound...]).trimmingCharacters(in: .whitespaces)))
                    } else {
                        all[all.count - 1].lines.append(("", body))
                    }
                } else if all[all.count - 1].tagline == nil {
                    all[all.count - 1].tagline = body
                }
            }
        }
        return all
    }

    var body: some View {
        List(releases) { release in
            Section {
                if let tagline = release.tagline { Text(tagline).font(.headline) }
                ForEach(Array(release.lines.enumerated()), id: \.offset) { _, line in
                    VStack(alignment: .leading, spacing: 2) {
                        if !line.title.isEmpty { Text(line.title).font(.subheadline.bold()) }
                        Text(line.text).font(.subheadline).foregroundStyle(line.title.isEmpty ? .secondary : .primary)
                    }
                }
            } header: {
                Text(release.version)
            }
        }
        .navigationTitle("What's new")
        .themed()
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
                if let settings {
                    let save = saved($settings, store) { try $0.setPlaybackSettings(value: $1) }
                    Toggle("Ask once a day", isOn: Binding(get: { settings.checkUpdates }, set: { var s = settings; s.checkUpdates = $0; save(s) }))
                }
            } footer: {
                Text("Asking sends nothing but the request to needle.nnx.fyi.")
            }
            Section {
                Toggle("Send crash reports", isOn: Binding(get: { crashReports }, set: { on in
                    crashReports = on
                    store.run { try $0.setCrashReports(on: on) }
                }))
            } footer: {
                Text("When Needle stops by mistake, it sends what went wrong (no music, names, or files) the next time it opens.")
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
