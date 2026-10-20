import CoreImage
import SwiftUI
import UniformTypeIdentifiers

/// Settings (⌘,), in sections as on the desktop.
struct SettingsWindow: View {
    private var store: Store { .shared }
    @AppStorage("settings-tab") private var tab = "Playback"

    static let tabs: [(String, String)] = [
        ("Playback", "speaker.wave.2"), ("Sound", "slider.vertical.3"), ("Library", "folder"),
        ("Appearance", "paintpalette"), ("Online services", "globe"), ("Discord", "bubble.left.and.bubble.right"),
        ("Stems", "square.stack.3d.up"), ("Plugins", "puzzlepiece.extension"), ("Your data", "externaldrive"),
        ("Keyboard", "command"),
    ]

    var body: some View {
        NavigationSplitView {
            List(selection: Binding<String?>(get: { tab }, set: { if let t = $0 { tab = t } })) {
                ForEach(Self.tabs, id: \.0) { name, icon in
                    Label(name, systemImage: icon).tag(name)
                }
            }
            .listItemTint(.fixed(store.playerAccent))
            .navigationSplitViewColumnWidth(200)
            .toolbar(removing: .sidebarToggle)
        } detail: {
            // Grouped boxes, as in System Settings; the title bar names the page.
            Form {
                switch tab {
                case "Sound": SoundPane()
                case "Library": LibrarySettings()
                case "Appearance": AppearanceSettings()
                case "Online services": ServicesSettings()
                case "Discord": DiscordSettings()
                case "Stems": StemsSettings()
                case "Plugins": PluginsSettings()
                case "Your data": DataSettings()
                case "Keyboard": KeyboardSettings()
                default: PlaybackSettingsView()
                }
                Section {
                } footer: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Needle \(store.version) · nnx")
                        Text("Library data: \(store.core.dataDirectory())").textSelection(.enabled)
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .formStyle(.grouped)
            .id(tab)
            .navigationTitle(tab)
        }
        .frame(minWidth: 820, idealWidth: 900, minHeight: 600, idealHeight: 720)
        .tint(store.playerAccent)
        .preferredColorScheme(store.plainColors.dark ? .dark : .light)
    }
}

/// One setting, as a row of a grouped form: its name and what it does, and its control on the
/// right.
struct SettingRow<Control: View>: View {
    let title: String
    var detail: String = ""
    @ViewBuilder let control: Control

    var body: some View {
        HStack(alignment: .center, spacing: 24) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                if !detail.isEmpty {
                    Text(detail).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 0)
            control
                .environment(\.settingTitle, title)
        }
        .padding(.vertical, 2)
    }
}

/// The header of a group of settings, with an optional line under it.
struct GroupHeader: View {
    let title: String
    var detail = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if !detail.isEmpty {
                Text(detail).font(.callout).fontWeight(.regular).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A switch bound to one of the core's settings, by its name.
/// A settings row's title, for the switch inside it to be named by (for VoiceOver).
private struct SettingTitle: EnvironmentKey {
    static let defaultValue = ""
}

extension EnvironmentValues {
    var settingTitle: String {
        get { self[SettingTitle.self] }
        set { self[SettingTitle.self] = newValue }
    }
}

/// A switch without a visible label, named by its row's title (or `label`) for VoiceOver.
struct RowToggle: View {
    @Environment(\.settingTitle) private var title
    var label: String?
    let isOn: Binding<Bool>

    var body: some View {
        Toggle(label ?? title, isOn: isOn)
    }
}

struct SettingSwitch: View {
    private var store: Store { .shared }
    let key: String
    let on: Bool

    var body: some View {
        RowToggle(isOn: Binding(get: { on }, set: { store.set([key: $0]) }))
            .toggleStyle(.switch)
            .labelsHidden()
    }
}

// MARK: - Playback

struct PlaybackSettingsView: View {
    private var store: Store { .shared }
    @State private var devices: [String] = []
    @State private var autoplay = ""
    @State private var members: [GroupMember] = []
    @State private var address: String?

    var body: some View {
        let s = store.settings
        Section {
            SettingRow(title: "ReplayGain", detail: "Evens out loudness using measured or tagged gain, with peak protection.") {
                SettingSwitch(key: "replay_gain", on: s.replayGain)
            }
            if s.replayGain {
                SettingRow(title: "Keep album dynamics", detail: "Uses album gain when an album has been measured, so quiet songs stay quiet next to loud ones. Falls back to track gain.") {
                    SettingSwitch(key: "album_gain", on: s.albumGain)
                }
            }
            SettingRow(title: "Crossfade", detail: "Songs overlap as one ends and the next begins. Tracks that follow each other on an album stay gapless.") {
                Picker("", selection: Binding(get: { Double(s.crossfade) }, set: { store.set(["crossfade": $0]) })) {
                    Text("Off").tag(0.0)
                    ForEach([2.0, 4, 6, 8, 12], id: \.self) { Text("\(Int($0)) s").tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 280)
            }
        } header: {
            GroupHeader(title: "Playback")
        }
        Section {
            device("System default", selected: s.outputDevice == nil, value: nil)
            if let current = s.outputDevice, current.hasPrefix("speaker") {
                device("A speaker on the network", selected: true, value: current)
            }
            ForEach(devices, id: \.self) { name in
                device(name, selected: s.outputDevice == name, value: name)
            }
        } header: {
            HStack(alignment: .firstTextBaseline) {
                GroupHeader(title: "Output device")
                Spacer()
                Button("Refresh") { loadDevices() }.buttonStyle(.link).font(.callout)
            }
        }
        Section {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    TextField("Rule", text: $autoplay, prompt: Text("rating >= 4 and not played(7d) shuffle limit 20")).labelsHidden()
                        .textFieldStyle(.roundedBorder)
                    Button("Save") {
                        let rule = autoplay.trimmingCharacters(in: .whitespaces)
                        if let error = store.core.checkQuery(query: rule).error, !rule.isEmpty {
                            store.fail(error)
                        } else {
                            store.set(["autoplay_query": rule])
                            store.notify("Autoplay rule saved.")
                        }
                    }
                }
            }
        } header: {
            GroupHeader(title: "When the queue runs out", detail: "Needle keeps playing tracks that match this rule. Leave it empty to stop.")
        }
        if !members.isEmpty {
            Section {
                ForEach(members, id: \.key) { member in
                    HStack {
                        Text(member.name).frame(width: 160, alignment: .leading).lineLimit(1)
                        Slider(value: Binding(get: { Double(member.delay) }, set: { setDelay(member.key, Int32($0)) }), in: -1000...1000, step: 10)
                        Text("\(member.delay > 0 ? "+" : "")\(member.delay) ms").monospacedDigit().frame(width: 72, alignment: .trailing)
                    }
                }
            } header: {
                GroupHeader(title: "Speaker timing", detail: "Needle lines the speakers up for you. If one still sounds early or late, move its slider: to the right plays it later.")
            }
        }
        Section {
            SettingRow(title: "Keep in the menu bar", detail: "Puts Needle in the menu bar. Closing the window hides it there and the music plays on; click the icon to play, pause, skip, show Needle, or quit.") {
                SettingSwitch(key: "tray", on: s.tray)
            }
            SettingRow(title: "Phone remote", detail: "Control Needle from a phone's web browser on the same Wi-Fi. Nothing to install on the phone.") {
                RowToggle(isOn: Binding(get: { s.remote }, set: { on in
                    store.set(["remote": on])
                    Task { try? await Task.sleep(for: .milliseconds(300)); address = store.core.remoteAddress() }
                }))
                .toggleStyle(.switch).labelsHidden()
            }
            if s.remote, let address {
                HStack(alignment: .top, spacing: 18) {
                    QRCode(text: address).frame(width: 150, height: 150)
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Point the phone's camera at the code").font(.body.weight(.medium))
                        Text("Or type this address in the phone's browser. Add it to the home screen to open it like an app.")
                            .font(.callout).foregroundStyle(.secondary)
                        Text(address).font(.callout.monospaced()).textSelection(.enabled)
                            .padding(6).background(store.plainColors.raised, in: RoundedRectangle(cornerRadius: 6))
                        HStack {
                            Button("Copy address") { store.copy(address); store.notify("Address copied.") }
                            Button("New address") {
                                store.run { try $0.newRemoteAddress() }
                                Task { try? await Task.sleep(for: .milliseconds(400)); self.address = store.core.remoteAddress() }
                                store.notify("The remote has a new address. Phones with the old one no longer work.")
                            }
                        }
                        Text("Anyone who has the address and can reach this Mac on a private network (your Wi-Fi, or a VPN) can control Needle and play its music. The address travels unencrypted, so use the remote only on networks you trust. Choose New address to lock others out.")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 6)
            }
        } header: {
            GroupHeader(title: "Menu bar and remote")
        }
        .task {
            autoplay = store.settings.autoplayQuery
            members = store.core.groupMembers()
            address = store.core.remoteAddress()
            loadDevices()
        }
    }

    private func loadDevices() {
        Task { devices = await store.load { $0.outputDevices() } ?? [] }
    }

    private func device(_ name: String, selected: Bool, value: String?) -> some View {
        Button {
            store.set(["output_device": value as Any? ?? NSNull()])
            members = store.core.groupMembers()
        } label: {
            HStack {
                Image(systemName: "hifispeaker").foregroundStyle(selected ? store.plainColors.accent : .secondary)
                Text(name).lineLimit(1)
                Spacer()
                if selected { Image(systemName: "checkmark").foregroundStyle(store.plainColors.accent) }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func setDelay(_ key: String, _ ms: Int32) {
        try? store.core.setSpeakerDelay(key: key, ms: ms)
        members = store.core.groupMembers()
    }
}

/// A QR code for the phone remote's address.
struct QRCode: View {
    let text: String

    var body: some View {
        if let image = Self.make(text) {
            Image(nsImage: image).interpolation(.none).resizable().padding(10).background(.white, in: RoundedRectangle(cornerRadius: 10))
        }
    }

    static func make(_ text: String) -> NSImage? {
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(text.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage else { return nil }
        let rep = NSCIImageRep(ciImage: output.transformed(by: CGAffineTransform(scaleX: 8, y: 8)))
        let image = NSImage(size: rep.size)
        image.addRepresentation(rep)
        return image
    }
}

// MARK: - Library

struct LibrarySettings: View {
    private var store: Store { .shared }
    @State private var roots: [String] = []
    @State private var measured: [UInt64] = [0, 0]
    @State private var hasFanart = false
    @State private var fanartKey = ""

    var body: some View {
        let s = store.settings
        Section {
            if roots.isEmpty { Text("No folders yet.").foregroundStyle(.secondary) }
            ForEach(roots, id: \.self) { root in
                HStack {
                    Image(systemName: "folder").foregroundStyle(.secondary)
                    Text(root).lineLimit(1).truncationMode(.middle)
                    Spacer()
                    Button { store.reveal(root) } label: { Image(systemName: "magnifyingglass") }.buttonStyle(.borderless).help("Show in Finder")
                    Button { remove(root) } label: { Image(systemName: "minus.circle") }.buttonStyle(.borderless).help("Stop reading this folder")
                }
            }
            HStack {
                Button { store.addFolder() } label: { Label("Add folder", systemImage: "plus") }
                Button("Check for changes") { store.rescan() }.disabled(store.scan.running)
            }
        } header: {
            GroupHeader(title: "Music folders", detail: "Needle watches these folders. It moves or removes files only when you ask, in Fix my library.")
        }
        Section {
            SettingRow(title: "Measure songs for radio", detail: measuredText) {
                SettingSwitch(key: "sound_analysis", on: s.soundAnalysis)
            }
        }
        Section {
            SettingRow(title: "Look things up online", detail: "Finds missing album covers (MusicBrainz and the Cover Art Archive), artist photos (Wikimedia Commons), and lyrics (LRCLIB). Only artist, album, title, and length are sent.") {
                SettingSwitch(key: "online_media", on: s.onlineMedia)
            }
            if s.onlineMedia {
                SettingRow(title: "Find missing album covers", detail: "Looks up every album without a cover. Large libraries take a while: MusicBrainz allows one request per second.") {
                    Button("Find covers") {
                        store.notify("Looking for missing album covers in the background…")
                        store.job { "Found covers for \(try $0.findAllCovers()) albums." }
                    }
                }
                SettingRow(title: "Artists' logos", detail: "With online lookups on, artist pages show the artist's logo from fanart.tv. Needle asks MusicBrainz for the artist's ID by name, and sends fanart.tv only that ID. Needle has a key of its own; your own key, if you add one, stays in the macOS Keychain.") {
                    if hasFanart {
                        Button("Remove your key", role: .destructive) {
                            Task {
                                if await store.ask({ try $0.setFanartKey(key: "") }) != nil {
                                    hasFanart = await store.load { $0.hasOwnFanartKey() } ?? false
                                }
                            }
                        }
                    } else {
                        HStack {
                            SecureField("fanart.tv key", text: $fanartKey).frame(width: 180)
                            Button("Save") {
                                let key = fanartKey
                                Task {
                                    if await store.ask({ try $0.setFanartKey(key: key) }) != nil { fanartKey = ""; hasFanart = true }
                                }
                            }
                            .disabled(fanartKey.trimmingCharacters(in: .whitespaces).isEmpty)
                        }
                    }
                }
            }
        } header: {
            GroupHeader(title: "Artwork and lyrics", detail: "Needle always uses covers, .lrc files, and lyrics tags found with your music.")
        }
        .task(id: store.libraryVersion) {
            roots = (try? store.core.folders()) ?? []
            measured = store.core.measuredCount()
            hasFanart = await store.load { $0.hasOwnFanartKey() } ?? false
        }
    }

    private var measuredText: String {
        let base = "Needle listens to each song on this Mac (tempo, key, energy, and tone) to build radio stations."
        let (done, total) = (Int(measured.first ?? 0), Int(measured.last ?? 0))
        if total == 0 { return base + " Nothing is sent anywhere." }
        return done < total ? base + " \(done.formatted()) of \(total.formatted()) measured." : base + " All \(total.formatted()) measured."
    }

    private func remove(_ root: String) {
        let alert = NSAlert()
        alert.messageText = "Stop reading “\((root as NSString).lastPathComponent)”?"
        alert.informativeText = "Its songs leave the library, with their plays, stars, and places in playlists. The files stay."
        alert.addButton(withTitle: "Stop Reading It")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.hasDestructiveAction = true
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        Task {
            if await store.ask({ try $0.removeFolder(path: root) }) != nil {
                store.core.rewatch()
                roots = (try? store.core.folders()) ?? []
                store.libraryChanged()
            }
        }
    }
}

// MARK: - Appearance

struct AppearanceSettings: View {
    private var store: Store { .shared }
    @State private var editing: ThemeFile?
    @State private var newTheme = false

    var body: some View {
        let s = store.settings
        Section {
            VStack(alignment: .leading, spacing: 10) {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 14)], alignment: .leading, spacing: 14) {
                    ForEach(Palette.bases, id: \.0) { mode, name, about in
                        ThemeCard(mode: mode, name: name, about: about, theme: nil)
                    }
                    ForEach(store.themes) { theme in
                        ThemeCard(mode: theme.id, name: theme.name, about: theme.readOnly ? "From \(theme.id.dropFirst(7).split(separator: "/").first ?? "")" : theme.author.isEmpty ? "Your theme" : "By \(theme.author)", theme: theme)
                            .contextMenu {
                                Button(theme.readOnly ? "Make a copy" : "Edit colors") { editing = theme }
                                Button("Show the file") { store.reveal(theme.path) }
                                if !theme.readOnly {
                                    Button("Delete", role: .destructive) { delete(theme) }
                                }
                            }
                    }
                }
                HStack {
                    Button("New theme…") { newTheme = true }
                    Button("Add a theme file…") { addTheme() }
                    Button("Open themes folder") {
                        let folder = store.core.themesFolder()
                        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
                        NSWorkspace.shared.open(URL(fileURLWithPath: folder))
                    }
                    if let theme = store.customTheme {
                        Button(theme.readOnly ? "Make a copy" : "Edit colors") { editing = theme }
                    }
                }
                .padding(.top, 4)
            }
            .padding(.vertical, 6)
        } header: {
            GroupHeader(title: "Look")
        }
        Section {
            SettingRow(title: "Ambient background", detail: "The cover that is playing (or your chosen color) fills the whole background, blurred. Works with every look.") {
                SettingSwitch(key: "ambient", on: s.ambient)
            }
            SettingRow(title: "Film grain", detail: "A fine texture over the whole window. Off by default.") {
                HStack {
                    Slider(value: Binding(get: { Double(s.grain) }, set: { store.set(["grain": $0]) }), in: 0...1).frame(width: 180)
                    Text(s.grain <= 0.001 ? "Off" : "\(Int(s.grain * 100))%").monospacedDigit().frame(width: 40, alignment: .trailing).foregroundStyle(.secondary)
                }
            }
            if !s.musicColors {
                SettingRow(title: "Color", detail: "Tints the buttons, highlights, and surfaces.") {
                    HStack(spacing: 6) {
                        ForEach(Palette.accents, id: \.0) { hex, name in
                            let color = Color(hex: hex) ?? Color(hex: "#e2b46c")!
                            Button { store.set(["accent_color": hex]) } label: {
                                Circle().fill(color).frame(width: 20, height: 20)
                                    .padding(3)
                                    .overlay(Circle().strokeBorder(s.accentColor == hex ? Color.primary : .clear, lineWidth: 2))
                            }
                            .buttonStyle(.plain)
                            .help(name)
                        }
                    }
                }
            }
            SettingRow(title: "Colors from the music", detail: "Tint Needle with the colors of the cover that is playing.") {
                SettingSwitch(key: "music_colors", on: s.musicColors)
            }
            SettingRow(title: "Colors from covers", detail: "Album and artist pages, and the whole window while one is open, take the colors of its cover or photo.") {
                SettingSwitch(key: "cover_colors", on: s.coverColors)
            }
            SettingRow(title: "Immersive cover", detail: "In the big player, the cover fills its side of the window edge to edge and fades into its colors, as in Apple Music.") {
                SettingSwitch(key: "immersive_cover", on: s.immersiveCover)
            }
            SettingRow(title: "Moving covers", detail: "A video beside the songs, such as cover.mp4, plays as the cover in the big player.") {
                SettingSwitch(key: "animated_covers", on: s.animatedCovers)
            }
            SettingRow(title: "Reduce motion", detail: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? "Your Mac has animations turned down, so Needle keeps still too." : "Turns off fades, slides, and other animations.") {
                SettingSwitch(key: "reduce_motion", on: s.reduceMotion)
            }
        } header: {
            GroupHeader(title: "Window")
        }
        Section {
            SettingRow(title: "Compact sidebar", detail: "Folding the sidebar leaves a strip of page icons, as in Apple Music, instead of hiding it altogether.") {
                RowToggle(isOn: Binding(get: { s.layout.compactSidebar }, set: { store.setLayout(["compact_sidebar": $0]) })).toggleStyle(.switch).labelsHidden()
            }
            SettingRow(title: "Fold the sidebar now", detail: "The same as the sidebar button in the toolbar.") {
                RowToggle(isOn: Binding(get: { s.layout.sidebarHidden }, set: { store.setLayout(["sidebar_hidden": $0]) })).toggleStyle(.switch).labelsHidden()
            }
            SettingRow(title: "Player in the title bar", detail: "The player sits in the bar at the top of the window, not at the foot of the page.") {
                RowToggle(isOn: Binding(get: { s.layout.playerOnTop }, set: { store.setLayout(["player_on_top": $0]) })).toggleStyle(.switch).labelsHidden()
            }
            SettingRow(title: "Hide the search field", detail: "Search from the command palette instead (⌘K, or ⌘F).") {
                RowToggle(isOn: Binding(get: { s.layout.searchHidden }, set: { store.setLayout(["search_hidden": $0]) })).toggleStyle(.switch).labelsHidden()
            }
        } header: {
            GroupHeader(title: "Layout")
        }
        Section {
            SettingRow(title: "Titles", detail: "The font for page titles, album and artist names, and the big player.") {
                Picker("", selection: Binding(get: { s.displayFont }, set: { store.set(["display_font": $0]) })) {
                    Text("System").tag("system")
                    Text("Condensed").tag("bahnschrift")
                    Text("Fraunces").tag("fraunces")
                }
                .pickerStyle(.segmented).labelsHidden().frame(width: 260)
            }
            SettingRow(title: "Density", detail: "Compact fits more tracks on screen.") {
                Picker("", selection: Binding(get: { s.layout.rowHeight < 50 }, set: { store.setLayout(["row_height": $0 ? 36 : 58]) })) {
                    Text("Compact").tag(true)
                    Text("Comfortable").tag(false)
                }
                .pickerStyle(.segmented).labelsHidden().frame(width: 220)
            }
            SettingRow(title: "Layout file", detail: "Share the sidebar, panels, and row height as a small JSON file.") {
                HStack {
                    Button("Import…") { importLayout() }
                    Button("Export…") { exportLayout() }
                }
            }
        } header: {
            GroupHeader(title: "Fonts and density")
        }
        .sheet(item: $editing) { theme in ThemeEditor(theme: theme).environment(store) }
        .sheet(isPresented: $newTheme) { ThemeEditor(theme: nil).environment(store) }
    }

    private func addTheme() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [UTType(filenameExtension: "toml") ?? .plainText]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        store.importTheme(url)
    }

    private func delete(_ theme: ThemeFile) {
        let id = theme.id
        store.run { try $0.deleteTheme(id: id) }
        if store.settings.theme == id { store.set(["theme": "dark"]) }
        Task { try? await Task.sleep(for: .milliseconds(200)); store.reloadThemes(); store.notify("Deleted the theme \(theme.name).") }
    }

    private func importLayout() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        guard panel.runModal() == .OK, let url = panel.url, let data = try? Data(contentsOf: url) else { return }
        let text = String(decoding: data, as: UTF8.self)
        do {
            try store.core.checkLayout(json: text)
            if let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] {
                store.set(["layout": object])
                store.notify("Layout applied.")
            }
        } catch {
            store.fail(error.message)
        }
    }

    private func exportLayout() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "needle-layout.json"
        guard panel.runModal() == .OK, let url = panel.url,
              let json = try? store.core.settingsJson(), let data = json.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let layout = object["layout"],
              let out = try? JSONSerialization.data(withJSONObject: layout, options: [.prettyPrinted, .sortedKeys]) else { return }
        do {
            try out.write(to: url)
            store.notify("Layout exported.")
        } catch {
            store.fail(error.message)
        }
    }
}

/// A small picture of a look, in today's colours, that switches to it when clicked.
struct ThemeCard: View {
    private var store: Store { .shared }
    let mode: String
    let name: String
    let about: String
    let theme: ThemeFile?

    var body: some View {
        let active = theme != nil ? store.settings.theme == mode : Palette.base(store.settings.theme) == Palette.base(mode) && !store.settings.theme.hasPrefix("custom:")
        let tint: HSL? = store.settings.musicColors ? store.coverTint.map(HSL.init) : Color(hex: store.settings.accentColor).map { HSL(NSColor($0)) }
        let look = theme.map { Palette.custom($0, tint: tint, ambient: false) } ?? Palette.build(Palette.base(mode), tint: tint, ambient: false)
        Button { choose() } label: {
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 6) {
                    VStack(alignment: .leading, spacing: 5) {
                        Capsule().fill(look.accent).frame(width: 22, height: 4)
                        ForEach([18, 20, 16], id: \.self) { w in Capsule().fill(look.ink3).frame(width: CGFloat(w), height: 4) }
                    }
                    .frame(width: 30)
                    .padding(.top, 4)
                    VStack(alignment: .leading, spacing: 5) {
                        Capsule().fill(look.ink).frame(width: 52, height: 4)
                        Capsule().fill(look.ink2).frame(width: 34, height: 4)
                        HStack(spacing: 5) {
                            ForEach([0.3, 0.6, 0.45], id: \.self) { _ in
                                RoundedRectangle(cornerRadius: 4).fill(LinearGradient(colors: [look.glow.opacity(0.9), look.chrome], startPoint: .topLeading, endPoint: .bottomTrailing))
                                    .frame(width: 22, height: 22)
                            }
                        }
                        .padding(.top, 3)
                        Capsule().fill(look.accent).frame(width: 30, height: 9)
                    }
                    .padding(7)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .background(look.canvas, in: RoundedRectangle(cornerRadius: 6))
                }
                .padding(6)
                .frame(width: 150, height: 96)
                .background(look.chrome, in: RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(active ? store.plainColors.accent : store.plainColors.line, lineWidth: 2))
                Text(name).font(.callout.weight(active ? .semibold : .regular)).lineLimit(1)
                Text(about).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(width: 150, alignment: .leading)
        }
        .buttonStyle(.plain)
    }

    private func choose() {
        var patch: [String: Any] = ["theme": mode]
        if let theme {
            if let grain = theme.grain { patch["grain"] = grain }
            if let font = theme.font { patch["display_font"] = font }
        }
        store.set(patch)
    }
}

/// A theme of the listener's own: its base, name, and the colours it changes.
struct ThemeEditor: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let theme: ThemeFile?
    @State private var name = ""
    @State private var author = ""
    @State private var base = "dark"
    @State private var musicColors = true
    @State private var colors: [String: Color] = [:]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(theme == nil ? "New theme" : theme!.readOnly ? "A copy of \(theme!.name)" : "Editing \(theme!.name)").font(.title2.weight(.semibold))
            Text("Colors you leave alone follow the base look and the colors you set. The same theme files work in Needle on every computer.")
                .font(.callout).foregroundStyle(.secondary)
            Form {
                TextField("Theme name", text: $name)
                TextField("Your name (optional)", text: $author)
                Picker("Starts from", selection: $base) {
                    Text("Night").tag("dark")
                    Text("Midnight").tag("midnight")
                    Text("Day").tag("light")
                }
                Toggle("Let the cover that is playing tint the colors this theme leaves alone", isOn: $musicColors)
                ForEach(Palette.slots, id: \.0) { key, label, detail in
                    HStack {
                        ColorPicker(label, selection: Binding(get: { colors[key] ?? fallback(key) }, set: { colors[key] = $0 }), supportsOpacity: false)
                            .help(detail)
                        if colors[key] != nil {
                            Button { colors[key] = nil } label: { Image(systemName: "arrow.uturn.backward") }
                                .buttonStyle(.borderless)
                                .help("Let this color follow the base look and the others")
                        }
                    }
                }
            }
            .formStyle(.grouped)
            HStack {
                if let theme, !theme.readOnly {
                    Button("Export…") { export(theme) }
                }
                Spacer()
                Button("Cancel") { dismiss() }
                Button("Save") { save() }.buttonStyle(.borderedProminent).disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 520, height: 680)
        .onAppear {
            guard let theme else { name = "My theme"; return }
            name = theme.readOnly ? "My \(theme.name)" : theme.name
            author = theme.author
            base = theme.base
            musicColors = theme.musicColors
            colors = theme.colors.compactMapValues(Color.init(hex:))
        }
    }

    private func fallback(_ key: String) -> Color {
        let p = Palette.build(Palette.base(base), tint: nil, ambient: false)
        return switch key {
        case "sidebar": p.chrome
        case "page": p.canvas
        case "card": p.raised
        case "card_hover": p.raisedHover
        case "border": p.line
        case "border_soft": p.lineSoft
        case "text_muted": p.ink2
        case "text_faint": p.ink3
        case "accent": p.accent
        case "accent_text": p.accentInk
        case "danger": p.danger
        case "success": p.success
        case "glow": p.glow
        default: p.ink
        }
    }

    private func save() {
        let id = theme?.readOnly == false ? theme!.id : ""
        let json = JSON.text(["id": id, "name": name, "base": base, "author": author, "music_colors": musicColors,
                              "colors": colors.mapValues(\.hex)] as [String: Any])
        Task {
            if let saved = await store.ask({ try $0.saveThemeJson(json: json) }) {
                store.reloadThemes()
                store.set(["theme": saved])
                store.notify("Saved \(name).")
                dismiss()
            }
        }
    }

    private func export(_ theme: ThemeFile) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "\(theme.name).toml"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        // Its own file chosen: it is already there (removing it first would lose it).
        if url.standardizedFileURL.resolvingSymlinksInPath() == URL(fileURLWithPath: theme.path).standardizedFileURL.resolvingSymlinksInPath() {
            store.notify("That is the theme's own file. Choose another place to save a copy.")
            return
        }
        do {
            if FileManager.default.fileExists(atPath: url.path) { try FileManager.default.removeItem(at: url) }
            try FileManager.default.copyItem(atPath: theme.path, toPath: url.path)
            store.notify("Saved \(url.lastPathComponent). Send it to a friend, or drop it on Needle to add it.")
        } catch {
            store.fail(error.message)
        }
    }
}

// MARK: - Discord

struct DiscordSettings: View {
    private var store: Store { .shared }

    var body: some View {
        let s = store.settings
        Section {
            SettingRow(title: "Show what you're playing on Discord", detail: "When Discord is open on this Mac, your profile shows what is playing and a time bar. Needle finds Discord by itself and only talks to the real Discord app on this Mac.") {
                SettingSwitch(key: "discord_presence", on: s.discordPresence)
            }
        }
        if s.discordPresence {
            Section {
                preview.padding(.vertical, 6).frame(maxWidth: .infinity)
                choice("Listening to …", "discord_title", s.discordTitle, [("song", "Song"), ("artist", "Artist"), ("needle", "Needle")])
                choice("First line", "discord_top", s.discordTop, [("artist", "Artist"), ("song", "Song"), ("album", "Album")])
                choice("Second line", "discord_middle", s.discordMiddle, [("song", "Song"), ("artist", "Artist"), ("album", "Album"), ("none", "Nothing")])
                choice("Third line", "discord_bottom", s.discordBottom, [("album", "Album"), ("song", "Song"), ("artist", "Artist"), ("none", "Nothing")])
            } header: {
                GroupHeader(title: "What your profile shows")
            }
            Section {
                SettingRow(title: "Look up covers", detail: "Discord can only show pictures from the web, so Needle finds the cover in Apple's iTunes catalog by artist and song name. Your files never leave this Mac. Turn this off to send no lookups at all.") {
                    SettingSwitch(key: "discord_covers", on: s.discordCovers)
                }
                SettingRow(title: "Show the Needle logo", detail: "A small badge on the cover, and the picture when there is no cover.") {
                    SettingSwitch(key: "discord_logo", on: s.discordLogo)
                }
                SettingRow(title: "Show while paused", detail: "Keeps the song on your profile, marked Paused, while the music is paused.") {
                    SettingSwitch(key: "discord_paused", on: s.discordPaused)
                }
                SettingRow(title: "Clear when nothing plays", detail: "After this long paused or stopped, the status goes away. It comes back when you play again.") {
                    Picker("", selection: Binding(get: { s.discordIdleMinutes }, set: { store.set(["discord_idle_minutes": $0]) })) {
                        Text("5 min").tag(5)
                        Text("10 min").tag(10)
                        Text("30 min").tag(30)
                        Text("1 hour").tag(60)
                        Text("Never").tag(0)
                    }
                    .pickerStyle(.segmented).labelsHidden().frame(width: 320)
                }
            }
        }
    }

    private func choice(_ title: String, _ key: String, _ value: String, _ options: [(String, String)]) -> some View {
        SettingRow(title: title) {
            Picker("", selection: Binding(get: { value }, set: { store.set([key: $0]) })) {
                ForEach(options, id: \.0) { Text($0.1).tag($0.0) }
            }
            .pickerStyle(.segmented).labelsHidden().frame(width: CGFloat(options.count) * 78)
        }
    }

    /// The card with the song playing, or an example.
    private var preview: some View {
        let s = store.settings
        let p = store.playback
        let (song, artist, album) = p?.hasSong == true ? (p!.title, p!.artist, p!.album) : ("Song name", "Artist", "Album")
        let text = { (field: String) -> String? in
            switch field {
            case "song": song
            case "artist": artist
            case "album": album.isEmpty ? nil : album
            case "needle": "Needle"
            default: nil
            }
        }
        return VStack(alignment: .leading, spacing: 8) {
            Text("Listening to \(text(s.discordTitle) ?? "Needle")").font(.caption.weight(.medium)).foregroundStyle(.secondary)
            HStack(spacing: 12) {
                if s.discordCovers, let art = p?.artwork {
                    Cover(path: art, seed: album, side: 64, corner: 8)
                } else if s.discordLogo {
                    RoundedRectangle(cornerRadius: 8).fill(Color(hex: "#1e1b18")!).frame(width: 64, height: 64)
                        .overlay { Image(systemName: "music.note").font(.title).foregroundStyle(Color(hex: "#e2b46c")!) }
                }
                VStack(alignment: .leading, spacing: 2) {
                    if let t = text(s.discordTop) { Text(t).font(.headline).lineLimit(1) }
                    if let t = text(s.discordMiddle) { Text(t).foregroundStyle(.secondary).lineLimit(1) }
                    if s.discordCovers || s.discordLogo, let t = text(s.discordBottom) { Text(t).foregroundStyle(.secondary).lineLimit(1) }
                }
            }
        }
        .padding(12)
        .frame(width: 340, alignment: .leading)
        .background(store.plainColors.raised, in: RoundedRectangle(cornerRadius: 10))
    }
}

// MARK: - Stems

struct StemsSettings: View {
    private var store: Store { .shared }
    @State private var info: StemsInfo?

    var body: some View {
        Section {
            if let info {
                SettingRow(title: "Split songs", detail: "\(Int(Double(info.used) / 1_048_576)) MB on this Mac.") {
                    Button("Delete all") {
                        store.core.deleteAllStems()
                        store.notify("All stems deleted.")
                        refresh()
                    }
                    .disabled(info.used == 0)
                }
                SettingRow(title: "Separation model", detail: info.modelReady ? "Downloaded (166 MB)." : "Not downloaded. It downloads the first time you split a song.") {
                    Button("Delete") {
                        store.core.deleteStemModel()
                        store.notify("Stem model deleted. It downloads again the next time you split a song.")
                        refresh()
                    }
                    .disabled(!info.modelReady)
                }
            }
        } header: {
            GroupHeader(title: "Stems", detail: "Split songs into drums, bass, vocals, and other, on this Mac.")
        } footer: {
            Text("Split a song from its menu (Split into Stems), or from the details panel. Stems use HT-Demucs, running locally. Nothing is uploaded.")
                .font(.callout).foregroundStyle(.secondary)
        }
        .task { refresh() }
    }

    private func refresh() { info = store.core.stemsInfo(id: "") }
}

// MARK: - Your data

struct DataSettings: View {
    private var store: Store { .shared }
    @State private var phrase = ""

    var body: some View {
        let s = store.settings
        Section {
            SettingRow(title: "Send crash reports", detail: "If Needle crashes, it sends a report the next time it starts: the Needle version, the system, and where in Needle it went wrong. File paths and your name are taken out first, and nothing is sent about your music or listening.") {
                SettingSwitch(key: "crash_reports", on: s.crashReports)
            }
            SettingRow(title: "Log file", detail: "Needle writes what it does, and any errors, to a log on this Mac. It never leaves this Mac unless you share it.") {
                HStack {
                    Button("Open log folder") { NSWorkspace.shared.open(URL(fileURLWithPath: store.core.logFolder())) }
                    Button("Copy error report") {
                        store.copy(store.core.errorReport())
                        store.notify("Copied an error report, without file paths or your name. Paste it in an email to dot@nnx.fyi.")
                    }
                }
            }
            SettingRow(title: "Welcome guide", detail: "The short guide Needle shows the first time it opens.") {
                Button("Show it again") { store.welcome = true }
            }
        } header: {
            GroupHeader(title: "Privacy and help")
        }
        Section {
            SettingRow(title: "Back up the library", detail: "Saves a consistent copy of the database, including recent changes.") {
                Button("Back up…") {
                    let panel = NSSavePanel()
                    panel.nameFieldStringValue = "needle-library.db"
                    guard panel.runModal() == .OK, let path = panel.url?.path else { return }
                    store.job { try $0.backupLibrary(path: path); return "Library backup saved." }
                }
            }
            SettingRow(title: "Import a playlist", detail: "Reads an M3U or M3U8 file. Add its music folder first so the tracks can be matched.") {
                Button("Import…") {
                    let panel = NSOpenPanel()
                    panel.allowedContentTypes = ["m3u", "m3u8"].compactMap { UTType(filenameExtension: $0) }
                    guard panel.runModal() == .OK, let path = panel.url?.path else { return }
                    store.job { try $0.importPlaylistFile(path: path) }
                }
            }
        } header: {
            GroupHeader(title: "Your library", detail: "History, ratings, and playlists live in one local database.")
        }
        Section {
            HStack {
                SecureField("Passphrase", text: $phrase, prompt: Text("Passphrase, at least 12 characters")).labelsHidden().textFieldStyle(.roundedBorder)
                Button("Export…") { transfer(export: true) }
                Button("Import…") { transfer(export: false) }
            }
        } header: {
            GroupHeader(title: "Move to another computer", detail: "Exports an encrypted bundle of history, ratings, and playlists. Import it where the same music files exist, using the same passphrase.")
        }
    }

    private func transfer(export: Bool) {
        guard phrase.count >= 12 else { store.fail("Use a passphrase of at least 12 characters."); return }
        let phrase = phrase
        if export {
            let panel = NSSavePanel()
            panel.nameFieldStringValue = "library.needle"
            guard panel.runModal() == .OK, let path = panel.url?.path else { return }
            store.job { try $0.exportSync(path: path, passphrase: phrase); return "Encrypted bundle exported. Audio files and credentials are not included." }
        } else {
            let panel = NSOpenPanel()
            panel.allowedContentTypes = [UTType(filenameExtension: "needle") ?? .data]
            guard panel.runModal() == .OK, let path = panel.url?.path else { return }
            store.job { core in
                let r = try core.importSync(path: path, passphrase: phrase)
                return "Imported \(r.listensAdded) listens and \(r.playlistsAdded) playlists. \(r.songsMatched) tracks matched; \(r.songsNotFound) need local copies."
            }
        }
    }
}

// MARK: - Keyboard

struct KeyboardSettings: View {
    private let keys: [(String, String)] = [
        ("Space", "Play or pause"), ("⌘ ← / →", "Previous or next track"), ("← / →", "Seek 10 seconds"),
        ("⌘ ↑ / ↓", "Volume"), ("↑ / ↓, ⇧", "Move or extend the selection"), ("↩", "Play the selection"),
        ("⌘K", "Command palette: go anywhere, do anything"), ("⌘F", "Search"), ("⇧↩", "Play next"),
        ("⌘↩", "Add to queue"), ("⌘E", "Edit tags"), ("⇧⌘D", "Favorite"), ("⌘J", "Show the queue"),
        ("⌘L", "Show the lyrics"), ("⌘I", "Show the details"), ("⌃⌘S", "Show or hide the sidebar"), ("⌘O", "Add a music folder"),
        ("⌘P", "Big player"), ("⌥⌘M", "Mini player"), ("⌃⌘F", "Full screen player"), ("⌘1 – 7", "Sidebar pages"),
        ("⌘,", "Settings"), ("⌘[ and ⌘]", "Back and forward"), ("⌘R", "Repeat"), ("Esc", "Close the big player"),
    ]

    var body: some View {
        Section {
            ForEach(keys, id: \.1) { key, what in
                LabeledContent(what) {
                    Text(key).font(.callout.monospaced())
                        .padding(.horizontal, 7).padding(.vertical, 2)
                        .background(.quaternary, in: RoundedRectangle(cornerRadius: 4))
                }
            }
        }
    }
}
