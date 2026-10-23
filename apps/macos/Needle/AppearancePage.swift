import SwiftUI
import UniformTypeIdentifiers

/// Settings › Appearance: each choice a small picture to click, in groups (the look, what sits
/// behind the pages, where things sit, the player and the songs, text and motion), as on the
/// desktop. The chosen card has an accent edge and a check.
struct AppearanceSettings: View {
    private var store: Store { .shared }
    @State private var editing: ThemeFile?
    @State private var newTheme = false

    var body: some View {
        let s = store.settings
        VStack(alignment: .leading, spacing: 34) {
            look(s).id("look")
            background(s).id("background")
            layout(s).id("layout")
            playing(s).id("playing")
            textAndMotion(s).id("text")
            layoutFile
        }
        .sheet(item: $editing) { theme in ThemeEditor(theme: theme).environment(store) }
        .sheet(isPresented: $newTheme) { ThemeEditor(theme: nil).environment(store) }
    }

    // MARK: Look

    private func look(_ s: NeedleSettings) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            GroupTitle(title: "Look", note: "Colors for the whole window")
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
            .controlSize(.small)
        }
    }

    // MARK: Background

    private func background(_ s: NeedleSettings) -> some View {
        let glass = !s.ambient && s.windowMaterial != "solid"
        let plain = !s.ambient && !glass
        return VStack(alignment: .leading, spacing: 14) {
            GroupTitle(title: "Background", note: "What sits behind the pages")
            HStack(alignment: .top, spacing: 14) {
                PickCard(title: "Plain", note: "The look's own colors", chosen: plain, height: 96) {
                    store.set(["ambient": false, "window_material": "solid"])
                } picture: { BackdropSketch(kind: .plain) }
                PickCard(title: "Ambient", note: "The playing cover, blurred", chosen: s.ambient, height: 96) {
                    store.set(["ambient": true])
                } picture: { BackdropSketch(kind: .ambient) }
                PickCard(title: "Glass", note: "Liquid Glass", chosen: glass, height: 96) {
                    store.set(["ambient": false, "window_material": "glass"])
                } picture: { BackdropSketch(kind: .glass) }
            }
            HStack(alignment: .top, spacing: 14) {
                ToggleTile(title: "Colors from the music", detail: "Tint Needle with the cover that plays.", on: s.musicColors,
                           set: { store.set(["music_colors": $0]) }) { TintPicture() }
                ToggleTile(title: "Colors from covers", detail: "Album and artist pages take their cover's colors.", on: s.coverColors,
                           set: { store.set(["cover_colors": $0]) }) { CoverColorsPicture() }
            }
            if !s.musicColors {
                Tile {
                    HStack(spacing: 14) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Color").font(.callout.weight(.semibold))
                            Text("Tints the buttons, highlights, and surfaces.").font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        ForEach(Palette.accents, id: \.0) { hex, name in
                            let color = Color(hex: hex) ?? Color(hex: "#e2b46c")!
                            Button { store.set(["accent_color": hex]) } label: {
                                Circle().fill(color).frame(width: 20, height: 20)
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
            GrainCard(amount: Double(s.grain))
        }
    }

    // MARK: Layout

    private func layout(_ s: NeedleSettings) -> some View {
        let compactRows = s.layout.rowHeight < 50
        return VStack(alignment: .leading, spacing: 14) {
            GroupTitle(title: "Layout", note: "Where things sit in the window")
            HStack(alignment: .top, spacing: 22) {
                Question(title: "The player") {
                    PickCard(title: "At the bottom", chosen: !s.layout.playerOnTop) {
                        store.setLayout(["player_on_top": false])
                    } picture: { WindowSketch(kind: .playerBottom) }
                    PickCard(title: "In the title bar", chosen: s.layout.playerOnTop) {
                        store.setLayout(["player_on_top": true])
                    } picture: { WindowSketch(kind: .playerTop) }
                }
                Question(title: "A folded sidebar") {
                    PickCard(title: "Hides", chosen: !s.layout.compactSidebar) {
                        store.setLayout(["compact_sidebar": false])
                    } picture: { WindowSketch(kind: .sidebarHidden) }
                    PickCard(title: "Icon strip", chosen: s.layout.compactSidebar) {
                        store.setLayout(["compact_sidebar": true])
                    } picture: { WindowSketch(kind: .sidebarStrip) }
                }
            }
            HStack(alignment: .top, spacing: 22) {
                Question(title: "Song rows") {
                    PickCard(title: "Roomy", chosen: !compactRows) {
                        store.setLayout(["row_height": 58])
                    } picture: { WindowSketch(kind: .rowsRoomy) }
                    PickCard(title: "Compact", chosen: compactRows) {
                        store.setLayout(["row_height": 36])
                    } picture: { WindowSketch(kind: .rowsCompact) }
                }
                Question(title: "The search field") {
                    PickCard(title: "In the sidebar", chosen: !s.layout.searchHidden) {
                        store.setLayout(["search_hidden": false])
                    } picture: { WindowSketch(kind: .searchSidebar) }
                    PickCard(title: "Only with ⌘K", chosen: s.layout.searchHidden) {
                        store.setLayout(["search_hidden": true])
                    } picture: { WindowSketch(kind: .searchPalette) }
                }
            }
            ToggleTile(title: "Fold the sidebar now", detail: "The same as the sidebar button in the toolbar.", on: s.layout.sidebarHidden,
                       set: { store.setLayout(["sidebar_hidden": $0]) }) { EmptyView() }
        }
    }

    // MARK: While music plays

    private func playing(_ s: NeedleSettings) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            GroupTitle(title: "While music plays", note: "The player and the songs")
            HStack(alignment: .top, spacing: 14) {
                ToggleTile(title: "Immersive cover", detail: "In the big player, the cover fills the left side and fades into its colors.",
                           on: s.immersiveCover, set: { store.set(["immersive_cover": $0]) }, height: 196) {
                    ImmersivePicture(on: s.immersiveCover)
                }
                .frame(maxWidth: .infinity)
                VStack(spacing: 14) {
                    ToggleTile(title: "Lyrics under the player", detail: "", on: s.playerLyrics,
                               set: { store.set(["player_lyrics": $0]) }, height: 52) { LyricsPicture(on: s.playerLyrics) }
                    ToggleTile(title: "Mark server songs", detail: "", on: s.serverMark,
                               set: { store.set(["server_mark": $0]) }, height: 52) { ServerMarkPicture(on: s.serverMark) }
                }
                .frame(width: 250)
            }
            ToggleTile(title: "Moving covers", detail: "A video beside the songs, such as cover.mp4, plays as the cover in the big player.",
                       on: s.animatedCovers, set: { store.set(["animated_covers": $0]) }) { EmptyView() }
        }
    }

    // MARK: Text and motion

    private func textAndMotion(_ s: NeedleSettings) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            GroupTitle(title: "Text and motion", note: "Titles and animations")
            HStack(alignment: .top, spacing: 14) {
                ForEach([("system", "System"), ("bahnschrift", "Condensed"), ("fraunces", "Fraunces")], id: \.0) { key, name in
                    PickCard(title: name, chosen: s.displayFont == key, height: 60) {
                        store.set(["display_font": key])
                    } picture: {
                        Text("Aa").font(Store.titleFont(key, size: 34)).frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
            }
            ToggleTile(title: "Reduce motion",
                       detail: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? "Your Mac has animations turned down, so Needle keeps still too." : "No fades, slides, or other animations.",
                       on: s.reduceMotion, set: { store.set(["reduce_motion": $0]) }) { EmptyView() }
        }
    }

    private var layoutFile: some View {
        HStack {
            Text("Layout file: the sidebar, panels, and row height, to share.").font(.callout).foregroundStyle(.secondary)
            Spacer()
            Button("Import…") { importLayout() }
            Button("Export…") { exportLayout() }
        }
        .controlSize(.small)
        .padding(.top, 10)
        .overlay(alignment: .top) { Divider() }
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

/// A group's title in the display font, with a short note beside it.
private struct GroupTitle: View {
    private var store: Store { .shared }
    let title: String
    let note: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(title).font(store.display(22, weight: .semibold))
            Text(note).font(.callout).foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// A question with its two answers side by side.
private struct Question<Cards: View>: View {
    let title: String
    @ViewBuilder let cards: Cards

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.subheadline).foregroundStyle(.secondary)
            HStack(alignment: .top, spacing: 12) { cards }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

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

/// A card's box: raised, with an accent edge when chosen or on.
private struct CardBox: ViewModifier {
    private var store: Store { .shared }
    let lit: Bool
    var badge = false

    func body(content: Content) -> some View {
        let p = store.plainColors
        let shape = RoundedRectangle(cornerRadius: 12, style: .continuous)
        content
            .background(p.raised, in: shape)
            .overlay(shape.strokeBorder(lit ? p.accent : p.line.opacity(0.7), lineWidth: lit ? 2 : 1))
            .overlay(alignment: .topTrailing) { if lit && badge { CheckBadge().offset(x: 7, y: -7) } }
            .contentShape(shape)
    }
}

/// A plain box for a row that is not a choice.
private struct Tile<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        content.padding(12).frame(maxWidth: .infinity, alignment: .leading).modifier(CardBox(lit: false))
    }
}

/// One answer: a picture and its name; a click chooses it.
private struct PickCard<Picture: View>: View {
    let title: String
    var note: String?
    let chosen: Bool
    var height: CGFloat = 78
    let action: () -> Void
    @ViewBuilder let picture: Picture

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 8) {
                picture
                    .frame(maxWidth: .infinity)
                    .frame(height: height)
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.callout.weight(.semibold)).lineLimit(1)
                    if let note { Text(note).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
            }
            .padding(9)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(CardBox(lit: chosen, badge: true))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(note.map { "\(title), \($0)" } ?? title)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// A setting that is on or off: a picture, its name, and a switch; a click anywhere on it
/// turns it over.
private struct ToggleTile<Picture: View>: View {
    let title: String
    let detail: String
    let on: Bool
    let set: (Bool) -> Void
    var height: CGFloat = 56
    @ViewBuilder let picture: Picture

    var body: some View {
        Button { set(!on) } label: {
            VStack(alignment: .leading, spacing: 10) {
                if ObjectIdentifier(Picture.self) != ObjectIdentifier(EmptyView.self) {
                    picture
                        .frame(maxWidth: .infinity)
                        .frame(height: height)
                        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
                HStack(alignment: .center, spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title).font(.callout.weight(.semibold))
                        if !detail.isEmpty {
                            Text(detail).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    Spacer(minLength: 8)
                    // Shown, not clicked: the whole tile is the button.
                    Toggle("", isOn: .constant(on)).toggleStyle(.switch).labelsHidden().controlSize(.small)
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(CardBox(lit: on))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityValue(on ? "On" : "Off")
        .accessibilityHint(detail)
        .accessibilityAddTraits(.isToggle)
    }
}

/// Film grain: a picture of it at the amount chosen, and a slider.
private struct GrainCard: View {
    private var store: Store { .shared }
    let amount: Double

    var body: some View {
        let p = store.plainColors
        Tile {
            HStack(spacing: 14) {
                ZStack {
                    LinearGradient(colors: [p.glow.opacity(0.9), p.chrome], startPoint: .topLeading, endPoint: .bottomTrailing)
                    Grain(amount: max(amount, 0.0001))
                }
                .frame(width: 52, height: 52)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Film grain").font(.callout.weight(.semibold))
                    Text(amount <= 0.001 ? "Off" : "\(Int(amount * 100))%").font(.caption).foregroundStyle(.secondary).monospacedDigit()
                }
                .frame(width: 90, alignment: .leading)
                Slider(value: Binding(get: { amount }, set: { store.set(["grain": $0]) }), in: 0...1)
                    .accessibilityLabel("Film grain")
            }
        }
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
private struct WindowSketch: View {
    enum Kind { case playerBottom, playerTop, sidebarHidden, sidebarStrip, rowsRoomy, rowsCompact, searchSidebar, searchPalette }
    let kind: Kind

    var body: some View {
        let ink = Ink.now
        HStack(spacing: 5) {
            switch kind {
            case .sidebarHidden: EmptyView()
            case .sidebarStrip:
                VStack(spacing: 5) {
                    ForEach(0..<4, id: \.self) { _ in Circle().fill(ink.accent).frame(width: 4, height: 4) }
                    Spacer()
                }
                .padding(.top, 6)
                .frame(width: 10)
                .background(ink.accent.opacity(0.18), in: RoundedRectangle(cornerRadius: 3))
            default:
                VStack(alignment: .leading, spacing: 4) {
                    if kind == .searchSidebar {
                        Capsule().fill(ink.accent).frame(width: 22, height: 5).padding(.bottom, 2)
                    }
                    ForEach([18, 14, 16, 12], id: \.self) { w in Capsule().fill(ink.part).frame(width: CGFloat(w), height: 3) }
                    Spacer()
                }
                .padding(.top, 6)
                .frame(width: 26, alignment: .leading)
            }
            VStack(alignment: .leading, spacing: 4) {
                if kind == .playerTop {
                    Capsule().fill(ink.accent).frame(height: 7)
                } else {
                    Capsule().fill(ink.part).frame(width: 34, height: 4)
                }
                content(ink)
                    .frame(maxHeight: .infinity, alignment: .top)
                if kind == .playerBottom {
                    Capsule().fill(ink.accent).frame(height: 7)
                } else if kind != .playerTop {
                    Capsule().fill(ink.soft).frame(height: 7)
                }
            }
        }
        .padding(7)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(ink.back)
        .overlay {
            if kind == .searchPalette {
                VStack(alignment: .leading, spacing: 4) {
                    Capsule().fill(ink.accent).frame(height: 5)
                    ForEach([0.8, 0.6, 0.7], id: \.self) { f in
                        GeometryReader { g in Capsule().fill(ink.accent.opacity(0.5)).frame(width: g.size.width * f, height: 3) }.frame(height: 3)
                    }
                }
                .padding(6)
                .frame(width: 70)
                .background(ink.back, in: RoundedRectangle(cornerRadius: 4))
                .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(ink.accent, lineWidth: 1))
            }
        }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func content(_ ink: Ink) -> some View {
        switch kind {
        case .rowsRoomy:
            VStack(alignment: .leading, spacing: 5) {
                ForEach(0..<3, id: \.self) { _ in
                    HStack(spacing: 5) {
                        RoundedRectangle(cornerRadius: 2).fill(ink.accent).frame(width: 11, height: 11)
                        VStack(alignment: .leading, spacing: 2) {
                            Capsule().fill(ink.part).frame(width: 40, height: 3)
                            Capsule().fill(ink.soft).frame(width: 28, height: 3)
                        }
                    }
                }
            }
        case .rowsCompact:
            VStack(alignment: .leading, spacing: 3.5) {
                ForEach(0..<6, id: \.self) { _ in
                    HStack(spacing: 5) {
                        RoundedRectangle(cornerRadius: 1).fill(ink.accent).frame(width: 5, height: 5)
                        Capsule().fill(ink.part).frame(width: 46, height: 2.5)
                    }
                }
            }
        default:
            HStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { _ in RoundedRectangle(cornerRadius: 3).fill(ink.soft) }
            }
        }
    }
}

/// What sits behind the pages: the look's own colors, the cover blurred, or glass.
private struct BackdropSketch: View {
    private var store: Store { .shared }
    enum Kind { case plain, ambient, glass }
    let kind: Kind
    private static let albums: [Color] = ["#c8553d", "#5b8c5a", "#e07a5f", "#b23a48", "#4a6fa5", "#d4a373", "#6d597a", "#3d6eb4"]
        .compactMap { Color(hex: $0) }

    var body: some View {
        let p = store.plainColors
        HStack(spacing: 0) {
            // The sidebar.
            VStack(alignment: .leading, spacing: 4) {
                ForEach([20, 14, 18, 12], id: \.self) { w in Capsule().fill(p.ink3.opacity(0.6)).frame(width: CGFloat(w), height: 3) }
                Spacer()
            }
            .padding(7)
            .frame(width: 38)
            .frame(maxHeight: .infinity)
            .background {
                if kind == .glass {
                    LinearGradient(colors: [Color(hex: "#7a8fb8")!.opacity(0.55), Color(hex: "#3d4a66")!.opacity(0.55)], startPoint: .top, endPoint: .bottom)
                        .overlay(Rectangle().strokeBorder(.white.opacity(0.25), lineWidth: 1))
                } else {
                    p.chrome
                }
            }
            // The page: album tiles and the player.
            VStack(spacing: 4) {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 4), count: 4), spacing: 4) {
                    ForEach(0..<8, id: \.self) { i in RoundedRectangle(cornerRadius: 2).fill(Self.albums[i]).aspectRatio(1, contentMode: .fit) }
                }
                Spacer(minLength: 0)
                Capsule().fill(p.ink3.opacity(0.35)).frame(height: 6)
                    .background(kind == .glass ? Color.white.opacity(0.12) : .clear, in: Capsule())
            }
            .padding(7)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background {
            switch kind {
            case .plain: p.canvas
            case .ambient:
                LinearGradient(colors: [p.glow.opacity(0.85), p.canvas], startPoint: .topLeading, endPoint: .bottomTrailing)
            case .glass:
                LinearGradient(colors: [Color(hex: "#2e3a52")!, Color(hex: "#1c2230")!], startPoint: .topLeading, endPoint: .bottomTrailing)
            }
        }
        .accessibilityHidden(true)
    }
}

/// The music's colors: a small player whose bar takes the playing cover's color.
private struct TintPicture: View {
    private var store: Store { .shared }

    var body: some View {
        let p = store.plainColors
        HStack(spacing: 10) {
            PlayingCover(side: 34)
            VStack(alignment: .leading, spacing: 5) {
                Capsule().fill(p.accent).frame(height: 5)
                Capsule().fill(p.ink3.opacity(0.4)).frame(width: 70, height: 4)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LinearGradient(colors: [p.glow.opacity(0.35), p.chrome], startPoint: .leading, endPoint: .trailing))
        .accessibilityHidden(true)
    }
}

/// Pages in their cover's colors: a cover over a band of its color.
private struct CoverColorsPicture: View {
    private var store: Store { .shared }

    var body: some View {
        let p = store.plainColors
        HStack(spacing: 10) {
            RoundedRectangle(cornerRadius: 4).fill(Color(hex: "#5b8c5a")!).frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 5) {
                Capsule().fill(.white.opacity(0.8)).frame(width: 60, height: 5)
                Capsule().fill(.white.opacity(0.45)).frame(width: 40, height: 4)
            }
            Spacer()
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LinearGradient(colors: [Color(hex: "#3f6b3e")!, p.chrome], startPoint: .leading, endPoint: .trailing))
        .accessibilityHidden(true)
    }
}

/// The playing song's cover (or a made-up one when nothing plays).
private struct PlayingCover: View {
    private var store: Store { .shared }
    let side: CGFloat
    var corner: CGFloat = 4

    var body: some View {
        let p = store.playback
        Cover(path: p?.hasSong == true ? p?.artwork : nil, seed: (p?.album ?? "") + (p?.artist ?? "Needle"), side: side, corner: corner)
    }
}

/// The big player with the immersive cover on (the cover fills the left side and fades) or off
/// (a small square cover), with the playing song's name.
private struct ImmersivePicture: View {
    private var store: Store { .shared }
    let on: Bool

    var body: some View {
        let p = store.plainColors
        let playback = store.playback
        let title = playback?.hasSong == true ? playback?.title ?? "" : "Your song"
        let artist = playback?.hasSong == true ? playback?.artist ?? "" : "Its artist"
        GeometryReader { g in
            HStack(spacing: 0) {
                if on {
                    Artwork(path: playback?.hasSong == true ? playback?.artwork : nil, seed: (playback?.album ?? "") + (playback?.artist ?? "Needle"), size: 600)
                        .frame(width: g.size.width * 0.46, height: g.size.height)
                        .clipped()
                        .overlay(alignment: .trailing) {
                            LinearGradient(colors: [p.chrome.opacity(0), p.chrome.opacity(0.75), p.chrome], startPoint: .leading, endPoint: .trailing).frame(width: g.size.width * 0.3)
                        }
                } else {
                    PlayingCover(side: g.size.height * 0.55, corner: 6)
                        .shadow(color: .black.opacity(0.35), radius: 8, y: 4)
                        .frame(width: g.size.width * 0.46, height: g.size.height)
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(store.display(17, weight: .semibold)).lineLimit(1)
                    Text(artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    ZStack(alignment: .leading) {
                        Capsule().fill(p.ink3.opacity(0.3))
                        Capsule().fill(p.accent).frame(width: 40)
                    }
                    .frame(height: 3)
                    .padding(.top, 6)
                }
                .padding(.horizontal, 14)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(p.chrome)
        .accessibilityHidden(true)
    }
}

/// A small player with, when on, the line being sung under it.
private struct LyricsPicture: View {
    private var store: Store { .shared }
    let on: Bool

    var body: some View {
        let p = store.plainColors
        let playback = store.playback
        VStack(spacing: 5) {
            HStack(spacing: 7) {
                PlayingCover(side: 16, corner: 3)
                Text(playback?.hasSong == true ? playback?.title ?? "" : "Your song").font(.caption2.weight(.semibold)).lineLimit(1)
                Spacer()
            }
            .padding(.horizontal, 7)
            .frame(height: 24)
            .background(p.raisedHover, in: RoundedRectangle(cornerRadius: 6))
            if on {
                Text("♪ the line being sung").font(.caption2).italic().foregroundStyle(p.ink2)
            } else {
                Capsule().fill(p.ink3.opacity(0.3)).frame(width: 40, height: 2).frame(height: 13)
            }
        }
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.chrome)
        .accessibilityHidden(true)
    }
}

/// Two song rows, with the server mark beside their titles when on.
private struct ServerMarkPicture: View {
    private var store: Store { .shared }
    let on: Bool

    var body: some View {
        let p = store.plainColors
        VStack(alignment: .leading, spacing: 5) {
            ForEach([("Opening", "#c8553d"), ("Low Tide", "#4a6fa5")], id: \.0) { title, hex in
                HStack(spacing: 6) {
                    RoundedRectangle(cornerRadius: 2).fill(Color(hex: hex)!).frame(width: 12, height: 12)
                    Text(title).font(.caption2.weight(.semibold))
                    if on { Image(systemName: "server.rack").font(.system(size: 8)).foregroundStyle(p.ink3) }
                    Spacer()
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(p.chrome)
        .accessibilityHidden(true)
    }
}
