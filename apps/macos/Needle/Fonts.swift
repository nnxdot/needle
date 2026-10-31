import AppKit
import SwiftUI

/// Needle's fonts (assets/fonts, SIL Open Font License, carried inside the app), by the
/// settings' names, as on every system: `display_font` for titles, `text_font` for the rest,
/// and "own:<family>" for a font on this Mac.
enum NeedleFonts {
    typealias Choice = (key: String, name: String, family: String?)

    static let titles: [Choice] = [
        ("system", "Plain", nil),
        ("poster", "Poster", "Roboto Condensed"),
        ("fraunces", "Fraunces", "Fraunces 72pt Soft"),
        ("instrument_serif", "Instrument Serif", "Instrument Serif"),
        ("dm_serif", "DM Serif Display", "DM Serif Display"),
        ("space_grotesk", "Space Grotesk", "Space Grotesk"),
        ("bricolage", "Bricolage Grotesque", "Bricolage Grotesque"),
        ("outfit", "Outfit", "Outfit"),
    ]

    static let texts: [Choice] = [
        ("system", "Plain", nil),
        ("inter", "Inter", "Inter"),
        ("atkinson", "Atkinson Hyperlegible", "Atkinson Hyperlegible"),
        ("lexend", "Lexend", "Lexend"),
        ("plex_mono", "IBM Plex Mono", "IBM Plex Mono"),
    ]

    /// Title fonts with one weight only: they are not made bold.
    static let singleWeight: Set<String> = ["Instrument Serif", "DM Serif Display"]

    /// A setting's family, or nil for Plain (the system's).
    static func family(_ key: String, in list: [Choice]) -> String? {
        if key.hasPrefix("own:") {
            let own = key.dropFirst(4).trimmingCharacters(in: .whitespaces)
            return own.isEmpty ? nil : own
        }
        // Bahnschrift, from before 2.0, is the narrow Poster now.
        let key = key == "bahnschrift" ? "poster" : key
        return list.first { $0.key == key }?.family
    }

    /// The name a setting shows: the font's own, or the family of one on this Mac.
    static func name(_ key: String, in list: [Choice]) -> String {
        if key.hasPrefix("own:") { return String(key.dropFirst(4)) }
        let key = key == "bahnschrift" ? "poster" : key
        return list.first { $0.key == key }?.name ?? list[0].name
    }

    /// The text font now, for `Font.needle`; nil for Plain. Set as the settings change.
    nonisolated(unsafe) static var text: String?

    /// The families on this Mac, for "A font on this Mac".
    static var installed: [String] { NSFontManager.shared.availableFontFamilies.filter { !$0.hasPrefix(".") } }
}

extension Font {
    /// A text style in the text font chosen (Settings › Appearance › Text), at the system's size
    /// for it; the system's own with Plain.
    static func needle(_ style: Font.TextStyle) -> Font {
        guard let family = NeedleFonts.text else { return .system(style) }
        let (size, weight): (CGFloat, Font.Weight) = switch style {
        case .largeTitle: (26, .regular)
        case .title: (22, .regular)
        case .title2: (17, .regular)
        case .title3: (15, .regular)
        case .headline: (13, .bold)
        case .subheadline: (11, .regular)
        case .callout: (12, .regular)
        case .footnote: (10, .regular)
        case .caption: (10, .regular)
        case .caption2: (10, .regular)
        default: (13, .regular)
        }
        return .custom(family, size: size).weight(weight)
    }
}

/// Titles or Text, as a row that opens: every font, each in itself, and one on this Mac.
struct FontRow: View {
    private var store: Store { .shared }
    let titles: Bool
    @State private var own = ""

    var body: some View {
        let list = titles ? NeedleFonts.titles : NeedleFonts.texts
        let key = titles ? store.settings.displayFont : store.settings.textFont
        let setting = titles ? "display_font" : "text_font"
        OpenRow(title: titles ? "Titles" : "Text",
                detail: titles ? "The font of page titles and big names." : "The font of lists, menus, and everything else.",
                icon: titles ? "textformat.size.larger" : "textformat",
                value: NeedleFonts.name(key, in: list)) {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(list, id: \.key) { choice in
                    let chosen = key == choice.key || (key == "bahnschrift" && choice.key == "poster")
                    Button { store.set([setting: choice.key]) } label: {
                        HStack(spacing: 12) {
                            Text(titles ? "Good evening" : "Harbor Lights · Mara Quinn · 3:48")
                                .font(sample(choice.family, titles: titles))
                                .lineLimit(1)
                            Spacer(minLength: 8)
                            Text(choice.name).font(.system(size: 11)).foregroundStyle(chosen ? store.colors.accent : store.colors.ink3)
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(chosen ? store.colors.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(choice.name)
                    .accessibilityAddTraits(chosen ? .isSelected : [])
                }
                HStack {
                    TextField("A font on this Mac", text: $own, prompt: Text("A font on this Mac, by its name"))
                        .textFieldStyle(.roundedBorder)
                        .onSubmit { useOwn(setting) }
                    Button("Use This Font") { useOwn(setting) }
                        .disabled(own.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                .padding(.top, 6)
                .onAppear { own = key.hasPrefix("own:") ? String(key.dropFirst(4)) : "" }
            }
        }
    }

    private func sample(_ family: String?, titles: Bool) -> Font {
        guard let family else { return titles ? .system(size: 22, weight: .semibold) : .system(size: 15) }
        let weight: Font.Weight = titles ? (NeedleFonts.singleWeight.contains(family) ? .regular : .semibold) : .regular
        return .custom(family, size: titles ? 22 : 15).weight(weight)
    }

    /// A family on this Mac, as typed: used when the Mac has it.
    private func useOwn(_ setting: String) {
        let typed = own.trimmingCharacters(in: .whitespaces)
        guard !typed.isEmpty else { return }
        guard let family = NeedleFonts.installed.first(where: { $0.caseInsensitiveCompare(typed) == .orderedSame }) else {
            store.fail("This Mac has no font named \(typed).")
            return
        }
        store.set([setting: "own:" + family])
    }
}
