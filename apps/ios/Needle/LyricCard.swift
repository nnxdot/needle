import SwiftUI

/// A lyric line as a picture to share, as on Windows: the cover at the top, the line large in
/// Needle's title font, then the song and Needle's mark at the foot, on the cover's colour. The
/// card is as tall as its words need (a square at least); short lines are written larger. Drawn
/// at 360 points wide, so a picture at 3× is 1080 pixels, as on Windows (whose numbers in pixels
/// are kept here, a third of each).
struct LyricCard: View {
    let line: String
    let title: String
    let artist: String
    let cover: CGImage?

    static let width: CGFloat = 360
    private static let margin = 84.0 / 3, coverSide = 300.0 / 3, foot = 200.0 / 3
    private static let amber = Color(red: 0.886, green: 0.706, blue: 0.424)

    /// The words' size (in pixels of the 1080 card) and rows: as large as the line allows,
    /// from 104 down to 52.
    static func layout(_ line: String) -> (size: Double, rows: [String]) {
        let room = 1080.0 - 2 * 84
        let length = textWidth(line)
        var size = 104.0
        while true {
            // About half the size per narrow letter, in this font.
            let rows = wrap(line, columns: Int(room / (size * 0.53)))
            // Short lines stay large; longer ones step down until they fit in a few rows.
            let most = length <= 30 ? 3 : length <= 90 ? 5 : 12
            if rows.count <= most || size <= 52 { return (size, Array(rows.prefix(12))) }
            size -= 6
        }
    }

    /// The card's height for a line, in points: a square at least.
    static func height(_ line: String, cover: Bool) -> CGFloat {
        let (size, rows) = layout(line.trimmingCharacters(in: .whitespacesAndNewlines))
        let wordsTop = 84 + (cover ? 300 + 90 : 0)
        let words = Double(rows.count) * (size + (size / 4).rounded(.down))
        return max(width, (Double(wordsTop) + words + 110 + 200) / 3)
    }

    var body: some View {
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        let (size, rows) = Self.layout(trimmed)
        let leading = (size + (size / 4).rounded(.down)) / 3
        let height = Self.height(trimmed, cover: cover != nil)
        let wordsTop = Self.margin + (cover != nil ? Self.coverSide + 30 : 0)
        let words = Double(rows.count) * leading
        let room = height - Self.foot - wordsTop
        let tone = Self.tone(cover)
        let who = [title, artist].filter { !$0.isEmpty }.joined(separator: " · ")
        ZStack(alignment: .topLeading) {
            LinearGradient(colors: [tone, tone.mix(with: .black, by: 0.55)], startPoint: .top, endPoint: .bottom)
            if let cover {
                Image(decorative: cover, scale: 1).resizable().scaledToFill()
                    .frame(width: Self.coverSide, height: Self.coverSide)
                    .clipShape(RoundedRectangle(cornerRadius: 28.0 / 3, style: .continuous))
                    .offset(x: Self.margin, y: Self.margin)
            }
            VStack(alignment: .leading, spacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    Text(row)
                        .font(Self.wide(row) ? .system(size: size / 3, weight: .bold) : .custom("Fraunces 72pt Soft", size: size / 3).weight(.semibold))
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                        .frame(height: leading, alignment: .center)
                }
            }
            .frame(width: Self.width - 2 * Self.margin, alignment: .leading)
            .offset(x: Self.margin, y: wordsTop + (room - words) / 2)
            VStack(alignment: .leading, spacing: 8) {
                Text(who)
                    .font(Self.wide(who) ? .system(size: 34.0 / 3, weight: .bold) : .custom("Inter", size: 34.0 / 3).weight(.semibold))
                    .opacity(0.85)
                    .lineLimit(1)
                HStack(spacing: 4) {
                    Circle().fill(Self.amber).frame(width: 22.0 / 3, height: 22.0 / 3)
                    Text("Needle").font(.custom("Inter", size: 10).weight(.semibold)).foregroundStyle(Self.amber)
                }
            }
            .frame(width: Self.width - 2 * Self.margin, alignment: .leading)
            .offset(x: Self.margin, y: height - Self.margin - 104.0 / 3)
        }
        .foregroundStyle(.white)
        .frame(width: Self.width, height: height, alignment: .topLeading)
        .clipped()
        .environment(\.colorScheme, .dark)
    }

    /// The cover's colour: its average, made deep enough for white words to read on it.
    static func tone(_ cover: CGImage?) -> Color {
        guard let cover, let context = CGContext(data: nil, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                                 space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            return Color(red: 46 / 255, green: 38 / 255, blue: 30 / 255)
        }
        context.interpolationQuality = .medium
        context.draw(cover, in: CGRect(x: 0, y: 0, width: 1, height: 1))
        guard let data = context.data else { return Color(red: 46 / 255, green: 38 / 255, blue: 30 / 255) }
        let p = data.bindMemory(to: UInt8.self, capacity: 4)
        let (r, g, b) = (Double(p[0]), Double(p[1]), Double(p[2]))
        let light = 0.2126 * r + 0.7152 * g + 0.0722 * b
        let scale = light > 70 ? 70 / light : 1
        return Color(red: r * scale / 255, green: g * scale / 255, blue: b * scale / 255)
    }

    /// How wide a letter is, in narrow letters: Japanese, Korean, and Chinese ones take two.
    static func letterWidth(_ c: Character) -> Int {
        guard let v = c.unicodeScalars.first?.value else { return 1 }
        switch v {
        case 0x1100...0x115F, 0x2E80...0xA4CF, 0xAC00...0xD7A3, 0xF900...0xFAFF, 0xFE30...0xFE4F,
             0xFF00...0xFF60, 0xFFE0...0xFFE6, 0x20000...0x3FFFD: return 2
        default: return 1
        }
    }

    static func textWidth(_ text: String) -> Int { text.reduce(0) { $0 + letterWidth($1) } }

    /// A row with wide letters is written in the system's font, bold (Needle's fonts have none).
    static func wide(_ row: String) -> Bool { row.contains { letterWidth($0) == 2 } }

    /// The line cut into rows at most `columns` narrow letters wide: at spaces, between wide
    /// letters (which need no spaces), and inside a word too long for a row.
    static func wrap(_ text: String, columns: Int) -> [String] {
        let width = max(columns, 2)
        // The pieces a row may break between: words, and each wide letter on its own.
        var pieces: [(String, Bool)] = []
        for word in text.split(whereSeparator: \.isWhitespace) {
            var spaced = true
            var part = ""
            for c in word {
                if letterWidth(c) == 2 {
                    if !part.isEmpty { pieces.append((part, spaced)); part = ""; spaced = false }
                    pieces.append((String(c), spaced))
                    spaced = false
                } else {
                    part.append(c)
                }
            }
            if !part.isEmpty { pieces.append((part, spaced)) }
        }
        var rows: [String] = []
        for (piece, spaced) in pieces {
            let gap = spaced ? 1 : 0
            if let last = rows.last, textWidth(last) + gap + textWidth(piece) <= width {
                rows[rows.count - 1] = last + (spaced ? " " : "") + piece
            } else {
                // A word wider than a row is cut.
                var row = ""
                for c in piece {
                    if !row.isEmpty && textWidth(row) + letterWidth(c) > width { rows.append(row); row = "" }
                    row.append(c)
                }
                rows.append(row)
            }
        }
        return rows
    }
}

/// A line chosen in the lyrics (held, then Share as Card).
struct CardLine: Identifiable {
    let id = UUID()
    let text: String
}

/// The card for a chosen line, and Share (Save Image is in the share sheet).
struct LyricCardSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let line: String
    let song: Song
    @State private var cover: CGImage?
    @State private var picture: UIImage?

    var body: some View {
        NavigationStack {
            VStack(spacing: 22) {
                // As tall as the line needs: shown smaller to fit.
                let height = LyricCard.height(line, cover: cover != nil)
                let fit = min(310 / LyricCard.width, 420 / height)
                card
                    .scaleEffect(fit, anchor: .topLeading)
                    .frame(width: LyricCard.width * fit, height: height * fit, alignment: .topLeading)
                    .clipShape(.rect(cornerRadius: 18))
                    .shadow(color: .black.opacity(0.3), radius: 20, y: 10)
                    .accessibilityLabel("A card of the line: \(line)")
                if let picture {
                    let image = Image(uiImage: picture)
                    ShareLink(item: image, preview: SharePreview(line, image: image)) {
                        Label("Share", systemImage: "square.and.arrow.up").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    .foregroundStyle(store.colors.onAccent)
                } else {
                    ProgressView()
                }
            }
            .padding(24)
            .navigationTitle("Lyric card")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
        .task {
            // The cover first, read whole, so the picture is made with it.
            if let path = song.artwork { cover = await Covers.load(path, size: 1080)?.cgImage }
            let renderer = ImageRenderer(content: card)
            renderer.scale = 3
            picture = renderer.uiImage
        }
    }

    private var card: LyricCard {
        LyricCard(line: line, title: song.title, artist: song.artist, cover: cover)
    }
}
