import SwiftUI
import UniformTypeIdentifiers

/// Settings › Appearance, calm, as an inset grouped list: pictures only for the looks, the
/// cover's shape and the buttons beside play as rows with two small sketches, and the rest as
/// rows with a small icon, one short line, and a switch.
struct AppearanceView: View {
    @Environment(Store.self) private var store
    @State private var addingTheme = false

    private let two = [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)]

    var body: some View {
        let app = store.app
        Form {
            Section {
                LazyVGrid(columns: two, spacing: 14) {
                    ForEach([("night", "Night", "Charcoal grey"), ("midnight", "Midnight", "True black"), ("day", "Day", "Light and airy")], id: \.0) { id, name, about in
                        LookCard(name: name, note: about, chosen: app.theme == id, colors: ThemeColors.base(id)) {
                            store.updateApp { $0.theme = id }
                        }
                    }
                    ForEach(store.themes, id: \.id) { theme in
                        LookCard(name: theme.name, note: theme.readOnly ? "Comes with Needle" : "Your theme", chosen: app.theme == theme.id, colors: colors(of: theme)) {
                            store.updateApp { $0.theme = theme.id }
                        }
                    }
                }
                .padding(.vertical, 6)
                NavigationLink(value: Route.themeEditor(nil)) { Label("New theme", systemImage: "plus") }
                Button("Add a theme file…", systemImage: "doc.badge.plus") { addingTheme = true }
                ForEach(store.themes.filter { !$0.readOnly }, id: \.id) { theme in
                    NavigationLink(value: Route.themeEditor(theme.id)) { Label("Edit \(theme.name)", systemImage: "pencil") }
                }
            } header: {
                Text("Look")
            }
            Section("Colours") {
                toggle("Colours from the song", "The app takes the playing song's colours.", "paintpalette.fill", .pink, \.ambientColors)
                toggle("Colours from covers", "Pages take their cover's colours.", "square.on.circle", .orange, \.coverColors)
                ColorPicker(selection: Binding(
                    get: { Color(hex: app.accentColor) ?? ThemeColors.pink },
                    set: { c in store.updateApp { $0.accentColor = c.hex } }), supportsOpacity: false) {
                    SettingLabel(title: "Accent", detail: app.accentColor.isEmpty ? "The theme's own" : "Your colour", icon: "circle.lefthalf.filled", tint: .blue)
                }
                if !app.accentColor.isEmpty {
                    Button("Use the theme's accent") { store.updateApp { $0.accentColor = "" } }
                }
            }
            Section("The player") {
                toggle("Immersive cover", "The cover fills the top of the player.", "rectangle.tophalf.inset.filled", .indigo, \.immersiveCover)
                if !app.immersiveCover {
                    SketchRow(title: "The cover", detail: "Its shape in the player.") {
                        SketchChoice(caption: "Square", chosen: app.coverShape != "round", part: .square) { store.updateApp { $0.coverShape = "square" } }
                        SketchChoice(caption: "Record", chosen: app.coverShape == "round", part: .record) { store.updateApp { $0.coverShape = "round" } }
                    }
                }
                SketchRow(title: "Beside play", detail: "The two buttons either side.") {
                    SketchChoice(caption: "Skip", chosen: app.sideButtons != "jump", part: .skip) { store.updateApp { $0.sideButtons = "skip" } }
                    SketchChoice(caption: "10 s", chosen: app.sideButtons == "jump", part: .jump) { store.updateApp { $0.sideButtons = "jump" } }
                }
                toggle("Cover behind the player", app.immersiveCover ? "Behind the lyrics and the queue, blurred." : "The player's background, blurred.", "photo.fill", .teal, \.coverBackdrop)
                if app.coverBackdrop {
                    toggle("Moving cover", "The blurred cover drifts slowly.", "wind", .teal, \.movingBackdrop)
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            SettingLabel(title: "Softness", icon: "drop.fill", tint: .teal)
                            Spacer()
                            Text("\(Int(app.backdropBlur))").foregroundStyle(.secondary).monospacedDigit()
                        }
                        Slider(value: Binding(get: { Double(app.backdropBlur) }, set: { v in store.updateApp { $0.backdropBlur = Float(v.rounded()) } }), in: 20...120)
                            .accessibilityLabel("Softness")
                    }
                }
                toggle("Moving covers", "A video, or an animated WebP or GIF, beside the songs.", "play.rectangle.fill", .purple, \.animatedCovers)
                toggle("Volume slider", "Under the controls.", "speaker.wave.2.fill", .gray, \.volumeSlider)
                toggle("Swipe the cover to skip", "Next or last song.", "hand.draw.fill", .gray, \.swipeCover)
                toggle("Open the player on play", "When you choose a song.", "arrow.up.forward.app.fill", .gray, \.openPlayerOnPlay)
            }
            Section("The mini player and lists") {
                toggle("Lyrics under the mini player", "The line being sung, in place of the artist.", "quote.bubble.fill", .pink, \.miniPlayerLyrics)
                toggle("Mark server songs", "A small server icon beside their artist.", "server.rack", .cyan, \.serverMark)
            }
            Section("Lyrics") {
                toggle("Live lyrics", "They move with the song, the line playing lit.", "text.quote", .orange, \.liveLyrics)
                toggle("Keep the screen on", "While the lyrics show.", "sun.max.fill", .yellow, \.keepScreenOnLyrics)
            }
            Section("Motion and touch") {
                toggle("Haptics", "A little tap on play, pause, and long presses.", "hand.tap.fill", .green, \.haptics)
                toggle("Reduce motion", "Needle's own switch, besides the iPhone's.", "figure.walk.motion", .blue, \.reduceMotion)
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

    private func toggle(_ title: String, _ detail: String, _ icon: String, _ tint: Color, _ key: WritableKeyPath<AppSettings, Bool>) -> some View {
        Toggle(isOn: Binding(get: { store.app[keyPath: key] }, set: { v in store.updateApp { $0[keyPath: key] = v } })) {
            SettingLabel(title: title, detail: detail, icon: icon, tint: tint)
        }
    }

    /// A theme file's colours, over its base.
    private func colors(of theme: ThemeInfo) -> ThemeColors {
        var c = ThemeColors.base(theme.base)
        if let v = theme.colors["page"].flatMap(Color.init(hex:)) { c.page = v }
        if let v = theme.colors["card"].flatMap(Color.init(hex:)) { c.card = v }
        if let v = theme.colors["text"].flatMap(Color.init(hex:)) { c.text = v }
        if let v = theme.colors["text_muted"].flatMap(Color.init(hex:)) { c.muted = v }
        if let v = theme.colors["accent"].flatMap(Color.init(hex:)) { c.accent = v }
        return c
    }
}

// MARK: - Pieces

/// The round accent check on a chosen card's corner.
private struct CheckBadge: View {
    @Environment(Store.self) private var store

    var body: some View {
        Image(systemName: "checkmark")
            .font(.system(size: 10, weight: .heavy))
            .foregroundStyle(.white)
            .frame(width: 22, height: 22)
            .background(store.colors.accent, in: .circle)
            .overlay(Circle().strokeBorder(store.colors.card, lineWidth: 2))
            .accessibilityHidden(true)
    }
}

/// A look: a phone in its colours and its name; a tap chooses it.
private struct LookCard: View {
    @Environment(Store.self) private var store
    let name: String
    let note: String
    let chosen: Bool
    let colors: ThemeColors
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                ThemeSketch(colors: colors)
                    .frame(height: 86)
                    .clipShape(.rect(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(chosen ? store.colors.accent : Color.secondary.opacity(0.25), lineWidth: chosen ? 2 : 1))
                    .overlay(alignment: .topTrailing) { if chosen { CheckBadge().offset(x: 6, y: -6) } }
                VStack(alignment: .leading, spacing: 1) {
                    Text(name).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Text(note).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .sensoryFeedback(.selection, trigger: chosen) { _, _ in store.app.haptics }
        .accessibilityLabel("\(name), \(note)")
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// A question as a row: its name and one line, and its two answers as small sketches.
private struct SketchRow<Choices: View>: View {
    let title: String
    let detail: String
    @ViewBuilder let choices: Choices

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
            Spacer(minLength: 8)
            HStack(spacing: 8) { choices }
        }
        .padding(.vertical, 2)
    }
}

/// One answer: a small grey sketch with only what changes in the accent, and its name.
private struct SketchChoice: View {
    @Environment(Store.self) private var store
    let caption: String
    let chosen: Bool
    let part: PlayerSketch.Part
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 3) {
                PlayerSketch(part: part)
                    .frame(width: 58, height: 52)
                    .clipShape(.rect(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(chosen ? store.colors.accent : Color.secondary.opacity(0.25), lineWidth: chosen ? 2 : 1))
                Text(caption).font(.caption2).foregroundStyle(chosen ? .primary : .secondary)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .sensoryFeedback(.selection, trigger: chosen) { _, _ in store.app.haptics }
        .accessibilityLabel(caption)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

// MARK: - Pictures

/// Greys for the sketches, and the accent for the one part that changes.
private struct Ink {
    let back: Color
    let part: Color
    let soft: Color
    let accent: Color

    @MainActor init(_ store: Store) {
        let c = store.colors
        back = c.dark ? Color(white: 0.14) : Color(white: 0.9)
        part = c.muted.opacity(0.5)
        soft = c.muted.opacity(0.22)
        accent = c.accent
    }
}

/// A phone in a look's own colours: a title, a row of cards, and an accent button.
private struct ThemeSketch: View {
    let colors: ThemeColors

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Capsule().fill(colors.text).frame(width: 42, height: 5)
            HStack(spacing: 5) {
                ForEach(0..<3, id: \.self) { _ in RoundedRectangle(cornerRadius: 4).fill(colors.card) }
            }
            .frame(height: 26)
            Capsule().fill(colors.muted.opacity(0.7)).frame(width: 54, height: 3)
            Capsule().fill(colors.accent).frame(width: 34, height: 10)
        }
        .padding(9)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(colors.page)
        .accessibilityHidden(true)
    }
}

/// The parts of the player's sketch an answer changes.
private struct PlayerSketch: View {
    @Environment(Store.self) private var store
    enum Part { case square, record, skip, jump }
    let part: Part

    var body: some View {
        let ink = Ink(store)
        VStack(spacing: 5) {
            switch part {
            case .square:
                RoundedRectangle(cornerRadius: 3).fill(ink.accent).frame(width: 26, height: 26)
            case .record:
                Circle().fill(ink.accent).frame(width: 26, height: 26)
                    .overlay(Circle().fill(ink.back).frame(width: 6, height: 6))
            case .skip, .jump:
                RoundedRectangle(cornerRadius: 3).fill(ink.soft).frame(width: 22, height: 22)
            }
            let sides = part == .skip || part == .jump
            HStack(spacing: 7) {
                Image(systemName: part == .jump ? "gobackward.10" : "backward.fill").foregroundStyle(sides ? ink.accent : ink.part)
                Image(systemName: "play.fill").foregroundStyle(ink.part)
                Image(systemName: part == .jump ? "goforward.10" : "forward.fill").foregroundStyle(sides ? ink.accent : ink.part)
            }
            .font(.system(size: sides ? 8 : 6, weight: .bold))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(ink.back)
        .accessibilityHidden(true)
    }
}
