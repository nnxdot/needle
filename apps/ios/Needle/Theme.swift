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

    /// Needle's amber, as on the Mac: on dark looks, and its deeper shade on Day.
    static let amber = Color(red: 0.886, green: 0.706, blue: 0.424)
    static let amberOnLight = Color(red: 0.549, green: 0.353, blue: 0.071)

    /// Text on the accent: dark on a light accent (amber, a pale cover), white on a deep one.
    var onAccent: Color {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(accent).getRed(&r, green: &g, blue: &b, alpha: &a)
        return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.6 ? Color(red: 0.11, green: 0.08, blue: 0.03) : .white
    }

    static func base(_ name: String) -> ThemeColors {
        switch name {
        case "day", "light":
            ThemeColors(page: Color(hex: "#F3F3F6")!, card: .white, text: Color(hex: "#111114")!, muted: Color(hex: "#6B6B75")!, accent: amberOnLight, dark: false)
        case "midnight":
            ThemeColors(page: .black, card: Color(hex: "#141418")!, text: Color(hex: "#F2F2F5")!, muted: Color(hex: "#8E8E98")!, accent: amber, dark: true)
        default:
            ThemeColors(page: Color(hex: "#0B0B10")!, card: Color(hex: "#1C1C22")!, text: Color(hex: "#F2F2F5")!, muted: Color(hex: "#9A9AA5")!, accent: amber, dark: true)
        }
    }

    /// Slots a theme file can set, as the desktop names them.
    static let slots: [(String, String)] = [
        ("page", "Page"), ("sidebar", "Side bar"), ("card", "Cards"), ("card_hover", "Cards, held"),
        ("text", "Text"), ("text_muted", "Quiet text"), ("accent", "Accent"), ("accent_text", "Text on the accent"),
        ("border", "Lines"), ("border_soft", "Soft lines"), ("danger", "Warnings"),
    ]
}

/// Needle's corners, as on the Mac (NEEDLE-2.0.md, rule 6): soft covers of 6 to 8 points, 12 in
/// the player, and three sizes for panes and cards. All are continuous, as the system's own.
enum Corner {
    /// A cover in a list, a grid, or on a card.
    static let cover: CGFloat = 8
    /// A small cover (the mini player's, a row's thumbnail, 44 points or less).
    static let smallCover: CGFloat = 6
    /// The player's own cover.
    static let player: CGFloat = 12
    /// Panes and cards: small (a chip's box, a tile), middle (a tile, a card), and large (a
    /// group of rows, a pane).
    static let small: CGFloat = 8
    static let middle: CGFloat = 14
    static let large: CGFloat = 22
}

extension Shape where Self == RoundedRectangle {
    /// A continuous rounded rectangle, for Needle's covers and panes (see `Corner`).
    static func soft(_ radius: CGFloat) -> RoundedRectangle {
        RoundedRectangle(cornerRadius: radius, style: .continuous)
    }
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

/// Film grain over the whole app (Settings › Appearance › Film grain), as on the Mac and
/// Android: a fixed noise tile, blended softly over everything, never in the way of a touch.
struct Grain: View {
    /// 0 to 1.
    let amount: Double

    private static let tile: UIImage = {
        let side = 128
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        var seed: UInt64 = 0x9E3779B97F4A7C15
        for i in stride(from: 0, to: pixels.count, by: 4) {
            seed = seed &* 6364136223846793005 &+ 1442695040888963407
            let v = UInt8(truncatingIfNeeded: seed >> 56)
            pixels[i] = v; pixels[i + 1] = v; pixels[i + 2] = v; pixels[i + 3] = 255
        }
        let provider = CGDataProvider(data: Data(pixels) as CFData)!
        let cg = CGImage(width: side, height: side, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: side * 4,
                         space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                         provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
        return UIImage(cgImage: cg, scale: 2, orientation: .up)
    }()

    var body: some View {
        Image(uiImage: Self.tile)
            .resizable(resizingMode: .tile)
            .blendMode(.overlay)
            .opacity(min(max(amount, 0), 1) * 0.35)
            .allowsHitTesting(false)
            .ignoresSafeArea()
            .accessibilityHidden(true)
    }
}

extension View {
    /// Film grain over this, when Settings › Appearance › Film grain is on.
    func grain(_ amount: Float) -> some View {
        overlay { if amount > 0.001 { Grain(amount: Double(amount)) } }
    }

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
    /// The theme as a file to share (its saved version), once it is written out.
    @State private var file: URL?

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
                Text("Colors")
            } footer: {
                Text("Colors you leave are the base's own. The same themes work in Needle on the desktop.")
            }
            if let id {
                Section {
                    // As on the Mac: the theme's file, to send to a friend or open in Needle
                    // on a computer.
                    if let file {
                        ShareLink(item: file) { Label("Export…", systemImage: "square.and.arrow.up") }
                    }
                    Button("Delete theme", role: .destructive) {
                        if store.app.theme == id { store.updateApp { $0.theme = "night" } }
                        Task {
                            _ = await store.ask { try $0.deleteTheme(id: id) }
                            store.reloadThemes()
                            dismiss()
                        }
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
        .task(id: store.themes.first { $0.id == id }) {
            guard let id, let export = await store.load({ try $0.exportTheme(id: id) }) else { file = nil; return }
            // In a folder of its own, so it is shared under the theme's own file name.
            let folder = FileManager.default.temporaryDirectory.appendingPathComponent("theme-export", isDirectory: true)
            try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let url = folder.appendingPathComponent(export.fileName)
            file = (try? export.text.write(to: url, atomically: true, encoding: .utf8)) != nil ? url : nil
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
