import SwiftUI
import UniformTypeIdentifiers

/// Settings › Appearance, calm, in the grouped form: pictures only for the look and the
/// background, the layout's choices as rows with two small sketches, and the rest as rows with
/// a switch.
struct AppearanceSettings: View {
    private var store: Store { .shared }
    @State private var editing: ThemeFile?
    @State private var newTheme = false

    var body: some View {
        let s = store.settings
        let glass = !s.ambient && s.windowMaterial != "solid"
        let compactRows = s.layout.rowHeight < 50
        Section {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 14, alignment: .top)], alignment: .leading, spacing: 16) {
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
            .padding(.vertical, 6)
            HStack {
                if let theme = store.customTheme {
                    Button(theme.readOnly ? "Make a copy" : "Edit colors") { editing = theme }
                }
                Button("New theme…") { newTheme = true }
                Spacer()
                Button("Add a theme file…") { addTheme() }
                Button("Open themes folder") {
                    let folder = store.core.themesFolder()
                    try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
                    NSWorkspace.shared.open(URL(fileURLWithPath: folder))
                }
            }
        } header: {
            GroupHeader(title: "Look")
        }
        Section {
            HStack(alignment: .top, spacing: 14) {
                PickCard(title: "Plain", note: "The look's own colors", chosen: !s.ambient && !glass) {
                    store.set(["ambient": false, "window_material": "solid"])
                } picture: { BackdropSketch(kind: .plain) }
                PickCard(title: "Ambient", note: "The cover, blurred", chosen: s.ambient) {
                    store.set(["ambient": true])
                } picture: { BackdropSketch(kind: .ambient) }
                PickCard(title: "Glass", note: "Liquid Glass", chosen: glass) {
                    store.set(["ambient": false, "window_material": "glass"])
                } picture: { BackdropSketch(kind: .glass) }
            }
            .padding(.vertical, 6)
            SettingRow(title: "Colors from the music", detail: "Tint Needle with the cover that plays.", icon: "paintpalette") {
                SettingSwitch(key: "music_colors", on: s.musicColors)
            }
            if !s.musicColors {
                SettingRow(title: "Color", detail: "Buttons, highlights, and surfaces.", icon: "circle.lefthalf.filled") {
                    HStack(spacing: 4) {
                        ForEach(Palette.accents, id: \.0) { hex, name in
                            let color = Color(hex: hex) ?? Color(hex: "#e2b46c")!
                            Button { store.set(["accent_color": hex]) } label: {
                                Circle().fill(color).frame(width: 18, height: 18)
                                    .padding(3)
                                    .overlay(Circle().strokeBorder(s.accentColor == hex ? Color.primary : .clear, lineWidth: 2))
                            }
                            .buttonStyle(.plain)
                            .help(name)
                            .accessibilityLabel(name)
                        }
                    }
                }
            }
            SettingRow(title: "Colors from covers", detail: "Album and artist pages take their cover's colors.", icon: "square.on.circle") {
                SettingSwitch(key: "cover_colors", on: s.coverColors)
            }
            SettingRow(title: "Film grain", detail: "A fine texture over the window.", icon: "circle.dotted") {
                HStack {
                    Slider(value: Binding(get: { Double(s.grain) }, set: { store.set(["grain": $0]) }), in: 0...1).frame(width: 160)
                        .accessibilityLabel("Film grain")
                    Text(s.grain <= 0.001 ? "Off" : "\(Int(s.grain * 100))%").monospacedDigit().frame(width: 40, alignment: .trailing).foregroundStyle(.secondary)
                }
            }
        } header: {
            GroupHeader(title: "Background")
        }
        Section {
            LayoutRow(title: "The player", detail: "At the foot of the window, or in the title bar.") {
                SketchChoice(caption: "Bottom", chosen: !s.layout.playerOnTop, kind: .playerBottom) { store.setLayout(["player_on_top": false]) }
                SketchChoice(caption: "Title bar", chosen: s.layout.playerOnTop, kind: .playerTop) { store.setLayout(["player_on_top": true]) }
            }
            LayoutRow(title: "A folded sidebar", detail: "What stays when you fold it.") {
                SketchChoice(caption: "Hides", chosen: !s.layout.compactSidebar, kind: .sidebarHidden) { store.setLayout(["compact_sidebar": false]) }
                SketchChoice(caption: "Icons", chosen: s.layout.compactSidebar, kind: .sidebarStrip) { store.setLayout(["compact_sidebar": true]) }
            }
            LayoutRow(title: "Song rows", detail: "Compact fits more songs on screen.") {
                SketchChoice(caption: "Roomy", chosen: !compactRows, kind: .rowsRoomy) { store.setLayout(["row_height": 58]) }
                SketchChoice(caption: "Compact", chosen: compactRows, kind: .rowsCompact) { store.setLayout(["row_height": 36]) }
            }
            LayoutRow(title: "The search field", detail: "In the sidebar, or only with ⌘K.") {
                SketchChoice(caption: "Sidebar", chosen: !s.layout.searchHidden, kind: .searchSidebar) { store.setLayout(["search_hidden": false]) }
                SketchChoice(caption: "⌘K", chosen: s.layout.searchHidden, kind: .searchPalette) { store.setLayout(["search_hidden": true]) }
            }
            SettingRow(title: "Fold the sidebar now", detail: "The same as the sidebar button.", icon: "sidebar.left") {
                RowToggle(isOn: Binding(get: { s.layout.sidebarHidden }, set: { store.setLayout(["sidebar_hidden": $0]) })).toggleStyle(.switch).labelsHidden()
            }
        } header: {
            GroupHeader(title: "Layout")
        }
        Section {
            SettingRow(title: "Immersive cover", detail: "In the big player, the cover fills the left side.", icon: "rectangle.lefthalf.inset.filled") {
                SettingSwitch(key: "immersive_cover", on: s.immersiveCover)
            }
            SettingRow(title: "Lyrics under the player", detail: "The line being sung, under the player.", icon: "quote.bubble") {
                SettingSwitch(key: "player_lyrics", on: s.playerLyrics)
            }
            SettingRow(title: "Mark server songs", detail: "A small server icon beside their title.", icon: "server.rack") {
                SettingSwitch(key: "server_mark", on: s.serverMark)
            }
            SettingRow(title: "Moving covers", detail: "A video, or an animated WebP or GIF, beside the songs.", icon: "play.rectangle") {
                SettingSwitch(key: "animated_covers", on: s.animatedCovers)
            }
        } header: {
            GroupHeader(title: "While music plays")
        }
        Section {
            SettingRow(title: "Titles", detail: "Page titles, names, and the big player.", icon: "textformat") {
                Picker("", selection: Binding(get: { s.displayFont }, set: { store.set(["display_font": $0]) })) {
                    Text("System").tag("system")
                    Text("Condensed").tag("bahnschrift")
                    Text("Fraunces").tag("fraunces")
                }
                .pickerStyle(.segmented).labelsHidden().frame(width: 250)
            }
            SettingRow(title: "Reduce motion",
                       detail: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? "Your Mac has animations turned down." : "No fades, slides, or other animations.",
                       icon: "figure.walk.motion") {
                SettingSwitch(key: "reduce_motion", on: s.reduceMotion)
            }
            SettingRow(title: "Layout file", detail: "The sidebar, panels, and row height, to share.", icon: "doc.badge.gearshape") {
                HStack {
                    Button("Import…") { importLayout() }
                    Button("Export…") { exportLayout() }
                }
            }
        } header: {
            GroupHeader(title: "Text and motion")
        }
        .sheet(item: $editing) { theme in ThemeEditor(theme: theme).environment(store) }
        .sheet(isPresented: $newTheme) { ThemeEditor(theme: nil).environment(store) }
    }

    // MARK: Actions

    private func addTheme() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [UTType(filenameExtension: "toml") ?? .plainText]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        store.importTheme(url)
    }

    private func delete(_ theme: ThemeFile) {
        let id = theme.id
        store.run { try $0.deleteTheme(id: id) }
        if store.settings.theme == id { store.chooseTheme("dark", extras: nil) }
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

// MARK: - Pieces

/// The round accent check on a chosen card's corner.
struct CheckBadge: View {
    private var store: Store { .shared }

    var body: some View {
        let p = store.plainColors
        Image(systemName: "checkmark")
            .font(.system(size: 9, weight: .heavy))
            .foregroundStyle(p.accentInk)
            .frame(width: 20, height: 20)
            .background(p.accent, in: Circle())
            .overlay(Circle().strokeBorder(p.chrome, lineWidth: 2))
            .accessibilityHidden(true)
    }
}

/// A background: a picture and its name; a click chooses it.
private struct PickCard<Picture: View>: View {
    private var store: Store { .shared }
    let title: String
    var note: String?
    let chosen: Bool
    let action: () -> Void
    @ViewBuilder let picture: Picture

    var body: some View {
        let p = store.plainColors
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                picture
                    .frame(maxWidth: .infinity)
                    .frame(height: 72)
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(chosen ? p.accent : p.line.opacity(0.7), lineWidth: chosen ? 2 : 1))
                    .overlay(alignment: .topTrailing) { if chosen { CheckBadge().offset(x: 7, y: -7) } }
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.callout.weight(chosen ? .semibold : .regular)).lineLimit(1)
                    if let note { Text(note).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(note.map { "\(title), \($0)" } ?? title)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// A layout question as a row: its name and one line, and its two answers as small sketches.
private struct LayoutRow<Choices: View>: View {
    let title: String
    let detail: String
    @ViewBuilder let choices: Choices

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(detail).font(.callout).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 12)
            HStack(spacing: 10) { choices }
        }
        .padding(.vertical, 4)
    }
}

/// One answer to a layout question: a small grey sketch with only what changes in the accent,
/// and its name; the chosen one has an accent edge.
private struct SketchChoice: View {
    private var store: Store { .shared }
    let caption: String
    let chosen: Bool
    let kind: WindowSketch.Kind
    let action: () -> Void

    var body: some View {
        let p = store.plainColors
        Button(action: action) {
            VStack(spacing: 4) {
                WindowSketch(kind: kind)
                    .frame(width: 76, height: 48)
                    .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(chosen ? p.accent : p.line.opacity(0.6), lineWidth: chosen ? 2 : 1))
                Text(caption).font(.caption).foregroundStyle(chosen ? .primary : .secondary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(caption)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

// MARK: - Pictures

/// Greys for the window sketches, and the accent for the one part that changes.
private struct Ink {
    let back: Color
    let part: Color
    let soft: Color
    let accent: Color

    @MainActor static var now: Ink {
        let p = Store.shared.plainColors
        return Ink(back: p.dark ? Color(white: 0.11) : Color(white: 0.9), part: p.ink3.opacity(0.42),
                   soft: p.ink3.opacity(0.2), accent: p.accent)
    }
}

/// A plain grey sketch of the window, with only the part an answer changes in the accent.
struct WindowSketch: View {
    enum Kind { case playerBottom, playerTop, sidebarHidden, sidebarStrip, rowsRoomy, rowsCompact, searchSidebar, searchPalette }
    let kind: Kind

    var body: some View {
        let ink = Ink.now
        HStack(spacing: 3) {
            switch kind {
            case .sidebarHidden: EmptyView()
            case .sidebarStrip:
                VStack(spacing: 3) {
                    ForEach(0..<4, id: \.self) { _ in Circle().fill(ink.accent).frame(width: 3, height: 3) }
                    Spacer()
                }
                .padding(.top, 4)
                .frame(width: 7)
                .background(ink.accent.opacity(0.18), in: RoundedRectangle(cornerRadius: 2))
            default:
                VStack(alignment: .leading, spacing: 3) {
                    if kind == .searchSidebar {
                        Capsule().fill(ink.accent).frame(width: 13, height: 3).padding(.bottom, 1)
                    }
                    ForEach([11, 8, 10], id: \.self) { w in Capsule().fill(ink.part).frame(width: CGFloat(w), height: 2) }
                    Spacer()
                }
                .padding(.top, 4)
                .frame(width: 15, alignment: .leading)
            }
            VStack(alignment: .leading, spacing: 3) {
                if kind == .playerTop {
                    Capsule().fill(ink.accent).frame(height: 5)
                } else {
                    Capsule().fill(ink.part).frame(width: 20, height: 3)
                }
                content(ink)
                    .frame(maxHeight: .infinity, alignment: .top)
                if kind == .playerBottom {
                    Capsule().fill(ink.accent).frame(height: 5)
                } else if kind != .playerTop {
                    Capsule().fill(ink.soft).frame(height: 5)
                }
            }
        }
        .padding(5)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(ink.back)
        .overlay {
            if kind == .searchPalette {
                VStack(alignment: .leading, spacing: 2) {
                    Capsule().fill(ink.accent).frame(height: 3)
                    Capsule().fill(ink.accent.opacity(0.5)).frame(width: 22, height: 2)
                    Capsule().fill(ink.accent.opacity(0.5)).frame(width: 16, height: 2)
                }
                .padding(4)
                .frame(width: 40)
                .background(ink.back, in: RoundedRectangle(cornerRadius: 3))
                .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(ink.accent, lineWidth: 1))
            }
        }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func content(_ ink: Ink) -> some View {
        switch kind {
        case .rowsRoomy:
            VStack(alignment: .leading, spacing: 3) {
                ForEach(0..<3, id: \.self) { _ in
                    HStack(spacing: 3) {
                        RoundedRectangle(cornerRadius: 1).fill(ink.accent).frame(width: 7, height: 7)
                        Capsule().fill(ink.part).frame(width: 26, height: 2)
                    }
                }
            }
        case .rowsCompact:
            VStack(alignment: .leading, spacing: 2) {
                ForEach(0..<6, id: \.self) { _ in
                    HStack(spacing: 3) {
                        RoundedRectangle(cornerRadius: 1).fill(ink.accent).frame(width: 3, height: 3)
                        Capsule().fill(ink.part).frame(width: 30, height: 1.5)
                    }
                }
            }
        default:
            HStack(spacing: 3) {
                ForEach(0..<3, id: \.self) { _ in RoundedRectangle(cornerRadius: 2).fill(ink.soft) }
            }
        }
    }
}

/// What sits behind the pages: the look's own colors, the cover blurred, or glass.
private struct BackdropSketch: View {
    private var store: Store { .shared }
    enum Kind { case plain, ambient, glass }
    let kind: Kind

    var body: some View {
        let p = store.plainColors
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 3) {
                ForEach([14, 10, 12], id: \.self) { w in Capsule().fill(p.ink3.opacity(0.6)).frame(width: CGFloat(w), height: 2) }
                Spacer()
            }
            .padding(6)
            .frame(width: 28)
            .frame(maxHeight: .infinity)
            .background {
                if kind == .glass {
                    LinearGradient(colors: [Color(hex: "#7a8fb8")!.opacity(0.55), Color(hex: "#3d4a66")!.opacity(0.55)], startPoint: .top, endPoint: .bottom)
                        .overlay(Rectangle().strokeBorder(.white.opacity(0.25), lineWidth: 1))
                } else {
                    p.chrome
                }
            }
            VStack(spacing: 4) {
                HStack(spacing: 4) {
                    ForEach(0..<3, id: \.self) { _ in RoundedRectangle(cornerRadius: 2).fill(p.ink3.opacity(0.28)) }
                }
                Capsule().fill(p.ink3.opacity(0.35)).frame(height: 5)
                    .background(kind == .glass ? Color.white.opacity(0.12) : .clear, in: Capsule())
            }
            .padding(6)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background {
            switch kind {
            case .plain: p.canvas
            case .ambient: LinearGradient(colors: [p.glow.opacity(0.85), p.canvas], startPoint: .topLeading, endPoint: .bottomTrailing)
            case .glass: LinearGradient(colors: [Color(hex: "#2e3a52")!, Color(hex: "#1c2230")!], startPoint: .topLeading, endPoint: .bottomTrailing)
            }
        }
        .accessibilityHidden(true)
    }
}
