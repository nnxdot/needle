import SwiftUI

/// The colours of the page, as Settings › Appearance sets them: Night, Midnight, Day, or a
/// theme made in the app or from a desktop theme file; with the accent chosen, or the playing
/// cover's colour.
struct ThemeColors {
    var page: Color
    var card: Color
    var text: Color
    var muted: Color
    var accent: Color
    var dark: Bool

    static let pink = Color(red: 1, green: 0.361, blue: 0.541)

    static func base(_ name: String) -> ThemeColors {
        switch name {
        case "day", "light":
            ThemeColors(page: Color(hex: "#F3F3F6")!, card: .white, text: Color(hex: "#111114")!, muted: Color(hex: "#6B6B75")!, accent: Color(hex: "#E0306A")!, dark: false)
        case "midnight":
            ThemeColors(page: .black, card: Color(hex: "#141418")!, text: Color(hex: "#F2F2F5")!, muted: Color(hex: "#8E8E98")!, accent: pink, dark: true)
        default:
            ThemeColors(page: Color(hex: "#0B0B10")!, card: Color(hex: "#1C1C22")!, text: Color(hex: "#F2F2F5")!, muted: Color(hex: "#9A9AA5")!, accent: pink, dark: true)
        }
    }

    /// Slots a theme file can set, as the desktop names them.
    static let slots: [(String, String)] = [
        ("page", "Page"), ("sidebar", "Side bar"), ("card", "Cards"), ("card_hover", "Cards, held"),
        ("text", "Text"), ("text_muted", "Quiet text"), ("accent", "Accent"), ("accent_text", "Text on the accent"),
        ("border", "Lines"), ("border_soft", "Soft lines"), ("danger", "Warnings"),
    ]
}

extension Store {
    var colors: ThemeColors {
        let custom = ["night", "midnight", "day"].contains(app.theme) ? nil : themes.first { $0.id == app.theme }
        var colors = ThemeColors.base(custom?.base ?? app.theme)
        if let c = custom?.colors {
            if let v = c["page"].flatMap(Color.init(hex:)) { colors.page = v }
            if let v = c["card"].flatMap(Color.init(hex:)) { colors.card = v }
            if let v = c["text"].flatMap(Color.init(hex:)) { colors.text = v }
            if let v = c["text_muted"].flatMap(Color.init(hex:)) { colors.muted = v }
            if let v = c["accent"].flatMap(Color.init(hex:)) { colors.accent = v }
        }
        if let chosen = Color(hex: app.accentColor) { colors.accent = chosen }
        if app.ambientColors, let cover = coverColor { colors.accent = cover }
        return colors
    }
}

extension Color {
    /// "#3e63dd" as a colour; nothing for anything but six hex digits.
    init?(hex: String) {
        let digits = hex.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: "#", with: "")
        guard digits.count == 6, let value = UInt32(digits, radix: 16) else { return nil }
        self.init(red: Double(value >> 16 & 0xFF) / 255, green: Double(value >> 8 & 0xFF) / 255, blue: Double(value & 0xFF) / 255)
    }

    var hex: String {
        let ui = UIColor(self)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        ui.getRed(&r, green: &g, blue: &b, alpha: &a)
        let clamp = { (v: CGFloat) in Int((min(1, max(0, v)) * 255).rounded()) }
        return String(format: "#%02X%02X%02X", clamp(r), clamp(g), clamp(b))
    }
}

/// The page under every screen: the theme's colour, with the playing cover's colour glowing
/// at the top when "colours from the song" is on.
struct PageBackground: View {
    @Environment(Store.self) private var store

    var body: some View {
        let colors = store.colors
        ZStack {
            colors.page
            if store.app.ambientColors, let cover = store.coverColor {
                RadialGradient(colors: [cover.opacity(colors.dark ? 0.35 : 0.22), .clear], center: .top, startRadius: 0, endRadius: 520)
            }
        }
        .ignoresSafeArea()
    }
}

extension View {
    /// The theme's page, text, and accent on a screen.
    func themed() -> some View {
        modifier(Themed())
    }
}

private struct Themed: ViewModifier {
    @Environment(Store.self) private var store

    func body(content: Content) -> some View {
        let colors = store.colors
        content
            .scrollContentBackground(.hidden)
            .background { PageBackground() }
            .foregroundStyle(colors.text)
            .tint(colors.accent)
    }
}

/// A theme of the listener's own: its base and the colours it changes.
struct ThemeEditorView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let id: String?
    @State private var name = ""
    @State private var base = "dark"
    @State private var colors: [String: Color] = [:]

    var body: some View {
        Form {
            Section {
                TextField("Name", text: $name)
                Picker("Starts from", selection: $base) {
                    Text("Night").tag("dark")
                    Text("Midnight").tag("midnight")
                    Text("Day").tag("light")
                }
            }
            Section {
                ForEach(ThemeColors.slots, id: \.0) { slot, label in
                    HStack {
                        ColorPicker(label, selection: Binding(
                            get: { colors[slot] ?? fallback(slot) },
                            set: { colors[slot] = $0 }), supportsOpacity: false)
                        if colors[slot] != nil {
                            Button { colors[slot] = nil } label: { Image(systemName: "arrow.uturn.backward") }
                                .buttonStyle(.borderless)
                        }
                    }
                }
            } header: {
                Text("Colours")
            } footer: {
                Text("Colours you leave are the base's own. The same themes work in Needle on the desktop.")
            }
            if let id {
                Section {
                    Button("Delete theme", role: .destructive) {
                        store.run { try $0.deleteTheme(id: id) }
                        if store.app.theme == id { store.updateApp { $0.theme = "night" } }
                        store.reloadThemes()
                        dismiss()
                    }
                }
            }
        }
        .navigationTitle(id == nil ? "New theme" : "Edit theme")
        .themed()
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") { save() }.disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .onAppear {
            guard let id, let theme = store.themes.first(where: { $0.id == id }) else { return }
            name = theme.name
            base = theme.base
            colors = theme.colors.compactMapValues(Color.init(hex:))
        }
    }

    private func fallback(_ slot: String) -> Color {
        let b = ThemeColors.base(base)
        return switch slot {
        case "page", "sidebar": b.page
        case "card", "card_hover", "border", "border_soft": b.card
        case "text_muted": b.muted
        case "accent": b.accent
        case "accent_text": b.dark ? .black : .white
        case "danger": .red
        default: b.text
        }
    }

    private func save() {
        let hexes = colors.mapValues(\.hex)
        let (id, name, base) = (id, name, base)
        Task {
            if let saved = await store.ask({ try $0.saveTheme(id: id, name: name, base: base, colors: hexes) }) {
                store.reloadThemes()
                store.updateApp { $0.theme = saved }
                dismiss()
            }
        }
    }
}
