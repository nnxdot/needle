import AppKit
import SwiftUI

/// Needle's colours, as on the desktop: a base look (Night, Midnight, Day) tinted by the playing
/// cover, or a theme file's colours over its base.
struct Palette: Equatable {
    var dark: Bool
    /// The sidebar and the bars.
    var chrome: Color
    /// The page the songs sit on.
    var canvas: Color
    /// Cards, fields, and rows under the pointer.
    var raised: Color
    var raisedHover: Color
    var line: Color
    var lineSoft: Color
    var ink: Color
    var ink2: Color
    var ink3: Color
    var accent: Color
    var accentInk: Color
    var danger: Color
    var success: Color
    /// The music's colour at full strength, for glows (never for text).
    var glow: Color

    var accentSoft: Color { accent.opacity(dark ? 0.15 : 0.11) }
    var selection: Color { accent.opacity(dark ? 0.14 : 0.12) }

    /// The base looks: settings value, name, and a few words.
    static let bases: [(String, String, String)] = [
        ("dark", "Night", "Charcoal grey"),
        ("midnight", "Midnight", "True black"),
        ("light", "Day", "Light and airy"),
    ]

    /// Colours to choose when colours from the music are off ("" is Needle's amber).
    static let accents: [(String, String)] = [
        ("", "Needle"), ("#e5484d", "Red"), ("#e93d82", "Pink"), ("#8e4ec6", "Purple"),
        ("#3e63dd", "Blue"), ("#12a594", "Teal"), ("#46a758", "Green"), ("#f76b15", "Orange"),
    ]

    /// Slots a theme file can set: key, name, and what it paints.
    static let slots: [(String, String, String)] = [
        ("sidebar", "Sidebar and bars", "The sidebar, the title bar, and the player at the bottom."),
        ("page", "Page", "The sheet the songs and pages sit on."),
        ("card", "Cards and fields", "Hovered rows, search fields, and cards."),
        ("card_hover", "Cards on hover", "Cards and buttons under the pointer."),
        ("border", "Lines", "Borders around fields and cards."),
        ("border_soft", "Soft lines", "Quiet dividers between rows and sections."),
        ("text", "Text", "Titles and most text."),
        ("text_muted", "Second text", "Artists, albums, and descriptions."),
        ("text_faint", "Faint text", "Small print, times, and hints."),
        ("accent", "Accent", "Buttons, switches, the playing song, and links."),
        ("accent_text", "Text on accent", "Labels on accent buttons."),
        ("danger", "Warnings", "Errors and delete buttons."),
        ("success", "Success", "Things that went well."),
        ("glow", "Glow", "The soft light behind covers and page headers."),
    ]

    enum Base { case night, midnight, day }

    static func base(_ name: String) -> Base {
        switch name {
        case "light", "day": .day
        case "midnight": .midnight
        default: .night
        }
    }

    /// A palette for `base`, tinted toward `tint` (a cover's colour) when there is one; `ambient`
    /// tints the surfaces much more strongly, for a background of the music's colour.
    static func build(_ base: Base, tint: HSL?, ambient: Bool) -> Palette {
        let tint = tint.flatMap { $0.s > 0.14 && $0.l > 0.06 && $0.l < 0.96 ? $0 : nil }
        let (hue, sat): (Double, Double) = switch (tint, base) {
        case (let t?, .day) where ambient: (t.h, 0.3)
        case (let t?, .day): (t.h, 0.16)
        case (let t?, _) where ambient: (t.h, 0.34)
        case (let t?, .midnight): (t.h, 0)
        case (let t?, _): (t.h, 0.13)
        case (nil, _) where ambient: (0.083, 0.2)
        case (nil, .midnight): (0, 0)
        case (nil, _): (0.083, 0.05)
        }
        let n = { (l: Double) in HSL(h: hue, s: sat, l: l) }
        let dark = base != .day
        let surfaces: [HSL] = switch base {
        case .night: [n(0.085), n(0.12), n(0.16), n(0.195), n(0.21), n(0.165)]
        case .midnight: [n(0), n(0.03), n(0.07), n(0.105), n(0.125), n(0.085)]
        case .day: [n(0.935), n(0.985), n(0.93), n(0.9), n(0.865), n(0.915)]
        }
        let inkSat = min(sat, 0.1)
        let inks: [HSL] = dark
            ? [HSL(h: hue, s: inkSat, l: 0.93), HSL(h: hue, s: inkSat, l: 0.72), HSL(h: hue, s: inkSat, l: 0.6)]
            : [HSL(h: hue, s: inkSat, l: 0.1), HSL(h: hue, s: inkSat, l: 0.31), HSL(h: hue, s: inkSat, l: 0.37)]
        let readable = { (c: HSL) in surfaces[0...2].allSatisfy { contrast(c, $0) >= 4.6 } }
        let accent: HSL
        if let t = tint {
            let s = min(0.85, max(0.5, t.s))
            var l = dark ? 0.64 : 0.4
            var color = HSL(h: t.h, s: s, l: l)
            while !readable(color), (0.05..<0.95).contains(l) {
                l += dark ? 0.02 : -0.02
                color = HSL(h: t.h, s: s, l: l)
            }
            accent = color
        } else {
            accent = dark ? HSL(hex: "#e2b46c")! : HSL(hex: "#8c5a12")!
        }
        let darkInk = HSL(h: accent.h, s: 0.5, l: 0.09)
        let accentInk = contrast(darkInk, accent) >= 4.5 ? darkInk : HSL(h: 0, s: 0, l: 1)
        let glow = tint.map { HSL(h: $0.h, s: min(0.9, max(0.4, $0.s)), l: dark ? 0.5 : 0.62) }
            ?? HSL(h: accent.h, s: accent.s, l: dark ? 0.45 : 0.7)
        return Palette(
            dark: dark, chrome: surfaces[0].color, canvas: surfaces[1].color, raised: surfaces[2].color,
            raisedHover: surfaces[3].color, line: surfaces[4].color, lineSoft: surfaces[5].color,
            ink: inks[0].color, ink2: inks[1].color, ink3: inks[2].color,
            accent: accent.color, accentInk: accentInk.color,
            danger: Color(hex: dark ? "#ee8479" : "#b4382c")!, success: Color(hex: dark ? "#8cc79a" : "#2f7a45")!,
            glow: glow.color)
    }

    /// A theme file's palette: its base (tinted by the music when the theme allows), with its
    /// colours on top; colours it leaves out follow the ones it sets.
    static func custom(_ theme: ThemeFile, tint: HSL?, ambient: Bool) -> Palette {
        let set = { (key: String) in theme.colors[key].flatMap(HSL.init(hex:)) }
        var base = Self.base(theme.base)
        if let page = set("page") {
            let l = luminance(page)
            if l < 0.2, base == .day { base = .night }
            if l >= 0.2, base != .day { base = .day }
        }
        let tint = theme.musicColors ? tint : nil
        var p = build(base, tint: tint, ambient: ambient && theme.musicColors)
        let mapping: [(String, WritableKeyPath<Palette, Color>)] = [
            ("sidebar", \.chrome), ("page", \.canvas), ("card", \.raised), ("card_hover", \.raisedHover),
            ("border", \.line), ("border_soft", \.lineSoft), ("text", \.ink), ("text_muted", \.ink2),
            ("text_faint", \.ink3), ("accent", \.accent), ("accent_text", \.accentInk), ("danger", \.danger),
            ("success", \.success), ("glow", \.glow),
        ]
        for (key, path) in mapping { if let c = set(key) { p[keyPath: path] = c.color } }
        if let page = set("page") {
            let dark = luminance(page) < 0.2
            p.dark = dark
            let toward = dark ? HSL(h: 0, s: 0, l: 1) : HSL(h: 0, s: 0, l: 0)
            if set("sidebar") == nil { p.chrome = page.mix(HSL(h: 0, s: 0, l: 0), dark ? 0.3 : 0.05).color }
            if set("card") == nil { p.raised = page.mix(toward, 0.05).color }
            if set("card_hover") == nil { p.raisedHover = page.mix(toward, 0.1).color }
            if set("border") == nil { p.line = page.mix(toward, 0.14).color }
            if set("border_soft") == nil { p.lineSoft = page.mix(toward, 0.08).color }
            if let ink = set("text") {
                if set("text_muted") == nil { p.ink2 = ink.mix(page, 0.25).color }
                if set("text_faint") == nil { p.ink3 = ink.mix(page, 0.38).color }
            }
        }
        if set("accent") != nil, set("glow") == nil, tint == nil { p.glow = p.accent }
        return p
    }

    /// An album's or artist's page: the window in the colour of its picture, with white or
    /// near-black words, whichever reads on it, and the buttons in those words' colour.
    static func paged(_ page: HSL) -> Palette {
        let dark = luminance(page) < 0.36
        let toward = dark ? HSL(h: 0, s: 0, l: 1) : HSL(h: 0, s: 0, l: 0)
        let ink = dark ? HSL(h: page.h, s: 0.06, l: 0.97) : HSL(h: page.h, s: 0.3, l: 0.07)
        var p = build(dark ? .night : .day, tint: page, ambient: true)
        p.dark = dark
        p.canvas = page.color
        p.chrome = page.mix(HSL(h: 0, s: 0, l: 0), dark ? 0.22 : 0.05).color
        p.raised = page.mix(toward, 0.08).color
        p.raisedHover = page.mix(toward, 0.14).color
        p.line = page.mix(toward, 0.2).color
        p.lineSoft = page.mix(toward, 0.11).color
        p.ink = ink.color
        p.ink2 = ink.mix(page, 0.3).color
        p.ink3 = ink.mix(page, 0.46).color
        p.accent = ink.color
        p.accentInk = page.mix(HSL(h: 0, s: 0, l: 0), dark ? 0.15 : 0).color
        p.glow = page.mix(toward, 0.25).color
        return p
    }

    static func luminance(_ c: HSL) -> Double {
        let (r, g, b) = c.rgb
        let channel = { (v: Double) in v <= 0.03928 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    static func contrast(_ a: HSL, _ b: HSL) -> Double {
        let (x, y) = (luminance(a), luminance(b))
        return (max(x, y) + 0.05) / (min(x, y) + 0.05)
    }
}

/// A colour by hue, saturation, and lightness, each 0 to 1, as the desktop works with them.
struct HSL: Equatable {
    var h: Double
    var s: Double
    var l: Double

    init(h: Double, s: Double, l: Double) {
        self.h = h; self.s = s; self.l = min(1, max(0, l))
    }

    init?(hex: String) {
        guard let c = Color(hex: hex) else { return nil }
        self.init(NSColor(c))
    }

    init(_ ns: NSColor) {
        let c = ns.usingColorSpace(.sRGB) ?? ns
        let (r, g, b) = (Double(c.redComponent), Double(c.greenComponent), Double(c.blueComponent))
        let (hi, lo) = (max(r, g, b), min(r, g, b))
        let l = (hi + lo) / 2
        var h = 0.0, s = 0.0
        if hi != lo {
            let d = hi - lo
            s = l > 0.5 ? d / (2 - hi - lo) : d / (hi + lo)
            switch hi {
            case r: h = (g - b) / d + (g < b ? 6 : 0)
            case g: h = (b - r) / d + 2
            default: h = (r - g) / d + 4
            }
            h /= 6
        }
        self.init(h: h, s: s, l: l)
    }

    var rgb: (Double, Double, Double) {
        if s == 0 { return (l, l, l) }
        let q = l < 0.5 ? l * (1 + s) : l + s - l * s
        let p = 2 * l - q
        let f = { (t0: Double) -> Double in
            var t = t0
            if t < 0 { t += 1 }
            if t > 1 { t -= 1 }
            if t < 1 / 6 { return p + (q - p) * 6 * t }
            if t < 1 / 2 { return q }
            if t < 2 / 3 { return p + (q - p) * (2 / 3 - t) * 6 }
            return p
        }
        return (f(h + 1 / 3), f(h), f(h - 1 / 3))
    }

    var color: Color {
        let (r, g, b) = rgb
        return Color(.sRGB, red: r, green: g, blue: b)
    }

    /// Part of the way toward another colour, in RGB as the desktop mixes.
    func mix(_ other: HSL, _ amount: Double) -> HSL {
        let (a, b) = (rgb, other.rgb)
        let t = { (x: Double, y: Double) in x + (y - x) * amount }
        return HSL(NSColor(srgbRed: t(a.0, b.0), green: t(a.1, b.1), blue: t(a.2, b.2), alpha: 1))
    }
}

extension Color {
    /// "#3e63dd" as a colour; nothing for anything but six hex digits.
    init?(hex: String) {
        let digits = hex.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: "#", with: "")
        guard digits.count == 6, let value = UInt32(digits, radix: 16) else { return nil }
        self.init(.sRGB, red: Double(value >> 16 & 0xFF) / 255, green: Double(value >> 8 & 0xFF) / 255, blue: Double(value & 0xFF) / 255)
    }

    var hex: String {
        let ns = NSColor(self).usingColorSpace(.sRGB) ?? .black
        let clamp = { (v: CGFloat) in Int((min(1, max(0, v)) * 255).rounded()) }
        return String(format: "#%02x%02x%02x", clamp(ns.redComponent), clamp(ns.greenComponent), clamp(ns.blueComponent))
    }
}

extension Store {
    /// The custom theme chosen, if one is.
    var customTheme: ThemeFile? {
        settings.theme.hasPrefix("custom:") ? themes.first { $0.id == settings.theme } : nil
    }

    /// The colours now: the look chosen, tinted by the playing cover when colours from the
    /// music are on, or by the colour chosen when they are off. (Kept in `colors`, which
    /// changes only when they do.)
    func lookNow() -> Palette {
        if settings.coverColors, let page = pageTint { return Palette.paged(HSL(page)) }
        let tint: HSL? = settings.musicColors ? coverTint.map(HSL.init) : Color(hex: settings.accentColor).map { HSL(NSColor($0)) }
        if let theme = customTheme { return Palette.custom(theme, tint: tint, ambient: settings.ambient) }
        return Palette.build(Palette.base(settings.theme), tint: tint, ambient: settings.ambient)
    }

    /// The font for page titles and big names.
    func display(_ size: CGFloat, weight: Font.Weight = .bold) -> Font {
        switch displayFont {
        case "fraunces": .custom(weight == .bold ? "Fraunces72ptSoft-Bold" : "Fraunces72ptSoft-SemiBold", size: size)
        case "bahnschrift": .system(size: size, weight: weight, design: .default).width(.condensed)
        default: .system(size: size, weight: weight, design: .default)
        }
    }


    /// Liquid Glass on the sidebar, the player, and the panels (off: solid bars).

}

/// How much of the page's right side the open side panel covers: pages that run a picture on
/// under its glass keep their words this far from the edge.
private struct PanelInset: EnvironmentKey {
    static let defaultValue: CGFloat = 0
}

extension EnvironmentValues {
    var panelInset: CGFloat {
        get { self[PanelInset.self] }
        set { self[PanelInset.self] = newValue }
    }
}

extension View {
    /// A pane of Liquid Glass (macOS 26), or the frosted material before it.
    @ViewBuilder
    func pane<S: Shape>(_ shape: S, tint: Color? = nil, interactive: Bool = false) -> some View {
        if #available(macOS 26, *) {
            glassEffect(interactive ? Glass.regular.tint(tint).interactive() : Glass.regular.tint(tint), in: shape)
        } else {
            background(.regularMaterial, in: shape)
        }
    }

    /// A glass button style on macOS 26, a bordered one before. The button keeps its own size:
    /// squeezed, the glass control's width kept changing, the layout never settled, and AppKit
    /// stopped the app.
    @ViewBuilder
    func glassButton(prominent: Bool = false) -> some View {
        if #available(macOS 26, *) {
            if prominent { buttonStyle(.glassProminent).fixedSize() } else { buttonStyle(.glass).fixedSize() }
        } else {
            if prominent { buttonStyle(.borderedProminent).fixedSize() } else { buttonStyle(.bordered).fixedSize() }
        }
    }
}

/// Film grain over the window: a still tile of noise.
struct Grain: View {
    let amount: Double
    private static let tile: NSImage = {
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
        return NSImage(cgImage: cg, size: NSSize(width: side, height: side))
    }()

    var body: some View {
        Image(nsImage: Self.tile)
            .resizable(resizingMode: .tile)
            .blendMode(.overlay)
            .opacity(amount * 0.35)
            .allowsHitTesting(false)
            .ignoresSafeArea()
    }
}
