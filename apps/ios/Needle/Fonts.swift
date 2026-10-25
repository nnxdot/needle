import CoreText
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Needle's fonts (assets/fonts, SIL Open Font License, carried inside the app), by the
/// settings' names, as on every system: `titleFont` for titles, `textFont` for the rest, and
/// "own:<family>" for a font file added on this iPhone.
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

    /// A setting's family, or nil for Plain. ("flex", Android's first title font, never meant
    /// anything on the iPhone: Plain.)
    static func family(_ key: String, in list: [Choice]) -> String? {
        if key.hasPrefix("own:") {
            let own = key.dropFirst(4).trimmingCharacters(in: .whitespaces)
            return own.isEmpty ? nil : own
        }
        return list.first { $0.key == key }?.family
    }

    static func name(_ key: String, in list: [Choice]) -> String {
        if key.hasPrefix("own:") { return String(key.dropFirst(4)) }
        return list.first { $0.key == key }?.name ?? list[0].name
    }

    /// The fonts now, for `Font.needle` and titles; nil for Plain. Set as the settings change.
    nonisolated(unsafe) static var text: String?
    nonisolated(unsafe) static var title: String?

    /// Font files added on this iPhone.
    static var ownFolder: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Needle/fonts", isDirectory: true)
    }

    /// Makes Needle's fonts, and the ones added on this iPhone, usable (once, at start).
    static func registerAll() {
        var files = Bundle.main.urls(forResourcesWithExtension: "ttf", subdirectory: nil) ?? []
        files += (try? FileManager.default.contentsOfDirectory(at: ownFolder, includingPropertiesForKeys: nil)) ?? []
        for file in files {
            CTFontManagerRegisterFontsForURL(file as CFURL, .process, nil)
        }
    }

    /// A .ttf or .otf file chosen in Files: kept in Needle's folder and made usable. Its family.
    static func add(_ url: URL) -> String? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let descriptors = CTFontManagerCreateFontDescriptorsFromURL(url as CFURL) as? [CTFontDescriptor],
              let first = descriptors.first,
              let family = CTFontDescriptorCopyAttribute(first, kCTFontFamilyNameAttribute) as? String else { return nil }
        try? FileManager.default.createDirectory(at: ownFolder, withIntermediateDirectories: true)
        let kept = ownFolder.appendingPathComponent(url.lastPathComponent)
        try? FileManager.default.removeItem(at: kept)
        guard (try? FileManager.default.copyItem(at: url, to: kept)) != nil else { return nil }
        CTFontManagerRegisterFontsForURL(kept as CFURL, .process, nil)
        return family
    }

    /// Takes the settings: the fonts for `Font.needle`, titles, and the bars' big titles.
    static func apply(title: String, text: String) {
        Self.title = family(title, in: titles)
        Self.text = family(text, in: texts)
        let bar = UINavigationBar.appearance()
        if let family = Self.title {
            bar.largeTitleTextAttributes = [.font: uiFont(family, size: 34, bold: !singleWeight.contains(family))]
        } else {
            bar.largeTitleTextAttributes = nil
        }
        if let family = Self.text {
            bar.titleTextAttributes = [.font: uiFont(family, size: 17, bold: true)]
        } else {
            bar.titleTextAttributes = nil
        }
    }

    private static func uiFont(_ family: String, size: CGFloat, bold: Bool) -> UIFont {
        let base = UIFontDescriptor(fontAttributes: [.family: family])
        let descriptor = bold ? base.addingAttributes([.traits: [UIFontDescriptor.TraitKey.weight: family == "Roboto Condensed" ? UIFont.Weight.black : UIFont.Weight.bold]]) : base
        return UIFont(descriptor: descriptor, size: size)
    }
}

extension Font {
    /// A text style in the text font chosen (Settings › Appearance › Text), at the system's
    /// size for it and growing with the reader's text size; the system's own with Plain.
    static func needle(_ style: Font.TextStyle) -> Font {
        guard let family = NeedleFonts.text else { return .system(style) }
        let (size, weight): (CGFloat, Font.Weight) = switch style {
        case .largeTitle: (34, .regular)
        case .title: (28, .regular)
        case .title2: (22, .regular)
        case .title3: (20, .regular)
        case .headline: (17, .semibold)
        case .subheadline: (15, .regular)
        case .callout: (16, .regular)
        case .footnote: (13, .regular)
        case .caption: (12, .regular)
        case .caption2: (11, .regular)
        default: (17, .regular)
        }
        return .custom(family, size: size, relativeTo: style).weight(weight)
    }

    /// A title (a section's, a page's name) in the title font chosen; the system's with Plain.
    static func needleTitle(size: CGFloat, relativeTo style: Font.TextStyle = .title2) -> Font {
        guard let family = NeedleFonts.title else { return .system(size: size, weight: .bold) }
        if NeedleFonts.singleWeight.contains(family) { return .custom(family, size: size, relativeTo: style) }
        return .custom(family, size: size, relativeTo: style).weight(family == "Roboto Condensed" ? .black : .bold)
    }
}

/// Settings › Appearance › Titles or Text: every font, each in itself, and a font file.
struct FontsView: View {
    @Environment(Store.self) private var store
    let titles: Bool
    @State private var importing = false

    var body: some View {
        let list = titles ? NeedleFonts.titles : NeedleFonts.texts
        let key = titles ? store.app.titleFont : store.app.textFont
        List {
            Section {
                ForEach(list, id: \.key) { choice in
                    let chosen = key == choice.key || (titles && key == "flex" && choice.key == "system")
                    Button { choose(choice.key) } label: {
                        HStack(spacing: 12) {
                            Text(titles ? "Good evening" : "Harbor Lights · Mara Quinn · 3:48")
                                .font(sample(choice.family))
                                .lineLimit(1)
                            Spacer(minLength: 8)
                            Text(choice.name).font(.caption).foregroundStyle(chosen ? AnyShapeStyle(.tint) : AnyShapeStyle(.secondary))
                        }
                        .contentShape(.rect)
                    }
                    .foregroundStyle(.primary)
                    .listRowBackground(chosen ? store.colors.accent.opacity(0.15) : nil)
                    .accessibilityLabel(choice.name)
                    .accessibilityAddTraits(chosen ? .isSelected : [])
                }
            }
            Section {
                if key.hasPrefix("own:") {
                    HStack {
                        Text(NeedleFonts.name(key, in: list)).font(sample(NeedleFonts.family(key, in: list)))
                        Spacer()
                        Image(systemName: "checkmark").foregroundStyle(.tint)
                    }
                }
                Button("Add a Font File…") { importing = true }
            } footer: {
                Text("A .ttf or .otf file from Files.")
            }
        }
        .navigationTitle(titles ? "Titles" : "Text")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .fileImporter(isPresented: $importing, allowedContentTypes: [.font, UTType(filenameExtension: "ttf") ?? .font, UTType(filenameExtension: "otf") ?? .font]) { result in
            guard case .success(let url) = result else { return }
            if let family = NeedleFonts.add(url) { choose("own:" + family) } else { store.message = "That file is not a font Needle can use" }
        }
    }

    private func choose(_ key: String) {
        store.updateApp { if titles { $0.titleFont = key } else { $0.textFont = key } }
    }

    private func sample(_ family: String?) -> Font {
        guard let family else { return titles ? .system(size: 22, weight: .semibold) : .system(size: 15) }
        if titles { return .custom(family, size: 22).weight(NeedleFonts.singleWeight.contains(family) ? .regular : .semibold) }
        return .custom(family, size: 15)
    }
}
