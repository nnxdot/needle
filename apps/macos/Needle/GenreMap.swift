import SwiftUI

/// A genre's bubble on the map: its centre and width, in points.
struct GenreBubble: Identifiable {
    let genre: String
    let songs: Int
    var x: Double
    var y: Double
    let size: Double
    var id: String { genre }
}

/// The genre map's layout, as on Windows: the 40 largest genres with a place as bubbles, as big
/// as their songs, nudged apart where they overlap; the rest (and those with no place yet) as
/// chips under the map.
enum GenreLayout {
    /// How many of the largest genres the map may show.
    static let most = 40

    /// The genres to draw on a map `width` by `height` points, and the ones left for the chips.
    /// `scale` makes the bubbles smaller on a small screen.
    static func lay(_ spots: [GenreSpot], width: Double, height: Double, scale: Double = 1)
        -> (bubbles: [GenreBubble], chips: [GenreSpot]) {
        // The core gives the genres with the most songs first.
        let biggest = Double(max(spots.first.map { Int($0.songs) } ?? 1, 1))
        var shown: [GenreSpot] = [], chips: [GenreSpot] = []
        for (i, spot) in spots.enumerated() {
            if i < most && spot.placed { shown.append(spot) } else { chips.append(spot) }
        }
        // Each bubble's centre and size; drawn largest first, so the small ones sit on top.
        var bubbles = shown.map { spot in
            GenreBubble(genre: spot.genre, songs: Int(spot.songs),
                        x: width * (0.06 + Double(spot.x) * 0.88),
                        y: height * (0.94 - Double(spot.y) * 0.88),
                        size: scale * (40 + 120 * (Double(spot.songs) / biggest).squareRoot()))
        }
        // Nudged apart where they overlap, to a 4 pt gap between edges, and kept inside the map.
        for _ in 0..<200 {
            for a in bubbles.indices {
                for b in bubbles.indices where b > a {
                    var dx = bubbles[b].x - bubbles[a].x, dy = bubbles[b].y - bubbles[a].y
                    // In one row or column (genres that sound alike): both ways, in turn.
                    if abs(dy) < 1 { dy = (a + b) % 2 == 0 ? 6 : -6 }
                    if abs(dx) < 1 { dx = b % 2 == 0 ? 6 : -6 }
                    let gap = (bubbles[a].size + bubbles[b].size) / 2 + 4
                    let far = (dx * dx + dy * dy).squareRoot()
                    guard far < gap else { continue }
                    // Apart along the line between them.
                    let (ux, uy) = far > 0.01 ? (dx / far, dy / far) : (1.0, 0.0)
                    let push = (gap - far) / 2
                    bubbles[a].x -= ux * push; bubbles[a].y -= uy * push
                    bubbles[b].x += ux * push; bubbles[b].y += uy * push
                }
            }
            for i in bubbles.indices {
                let r = bubbles[i].size / 2
                bubbles[i].x = min(max(bubbles[i].x, r + 4), width - r - 4)
                bubbles[i].y = min(max(bubbles[i].y, r + 4), height - r - 4)
            }
        }
        return (bubbles, chips)
    }

    /// A genre's colour, from its name as on Windows: a hue from a hash of the lowercase name,
    /// lighter on dark looks.
    static func tone(_ genre: String, dark: Bool) -> Color {
        let hash = genre.lowercased().utf8.reduce(UInt32(2166136261)) { ($0 ^ UInt32($1)) &* 16777619 }
        // Saturation 0.55 and lightness 0.6 or 0.42 (HSL), as hue, saturation, and brightness.
        let light = dark ? 0.6 : 0.42
        let value = light + 0.55 * min(light, 1 - light)
        return Color(hue: Double(hash % 360) / 360, saturation: 2 * (1 - light / value), brightness: value)
    }

    /// The smallest and largest name sizes.
    static let smallest = 11.0, largest = 17.0

    /// How wide the longest word of a name is (a hyphen may end a line), in points, at `font` size.
    static func widest(_ genre: String, font: Double) -> Double {
        let face = NSFont.systemFont(ofSize: font, weight: .semibold)
        return genre.replacingOccurrences(of: "-", with: "- ").split(separator: " ").map { Double((String($0) as NSString).size(withAttributes: [.font: face]).width) }.max() ?? 0
    }

    /// A bubble's name size: a seventh of the bubble, as on Windows, or smaller so its longest
    /// word fits on one line.
    static func label(_ genre: String, size: Double) -> Double {
        let fit = smallest * (size - 10) / max(widest(genre, font: smallest), 1)
        return min(min(max(size / 7, smallest), largest), max(fit, smallest * 0.75))
    }

    /// Whether a bubble is big enough to say its number of songs too.
    static func roomy(_ bubble: GenreBubble) -> Bool { bubble.size > 74 }
}

/// Chips that wrap onto as many lines as they need.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 600
        var x: CGFloat = 0, y: CGFloat = 0, line: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width { x = 0; y += line + spacing; line = 0 }
            x += size.width + spacing
            line = max(line, size.height)
        }
        return CGSize(width: width, height: y + line)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, line: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX { x = bounds.minX; y += line + spacing; line = 0 }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            line = max(line, size.height)
        }
    }
}

/// The genre map: a bubble for each genre, as big as its number of songs, where its songs
/// sound (dark to bright across, calm to lively up); a click plays it shuffled. The genres past
/// the 40 largest, and those with no song measured yet, are chips under it.
struct GenreMapPage: View {
    private var store: Store { .shared }
    @State private var spots: [GenreSpot]?
    @State private var hovered: String?
    @State private var width: Double = 900

    var body: some View {
        let colors = store.colors
        // As on Windows: as wide as the page allows, a little over half as tall.
        let side = min(max(width - 56, 280), 980)
        let height = min(max(side * 0.62, 280), 600)
        let laid = GenreLayout.lay(spots ?? [], width: side, height: height)
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Genre map").font(store.display(28))
                    Text(summary(laid.bubbles.isEmpty)).font(.needle(.callout)).foregroundStyle(colors.ink2)
                }
                if !laid.bubbles.isEmpty {
                    map(laid.bubbles, width: side, height: height)
                }
                if !laid.chips.isEmpty {
                    Text("More genres").font(store.display(18))
                    FlowLayout {
                        ForEach(laid.chips, id: \.genre) { chip in
                            Button { play(chip.genre) } label: {
                                HStack(spacing: 8) {
                                    Text(chip.genre)
                                    Text(Int(chip.songs).formatted()).foregroundStyle(colors.ink3)
                                }
                                .font(.needle(.callout))
                                .padding(.horizontal, 12).padding(.vertical, 5)
                                .background(colors.raised, in: Capsule())
                                .overlay(Capsule().strokeBorder(colors.ink3.opacity(0.25)))
                            }
                            .buttonStyle(.plain)
                            .help("Play \(chip.genre), shuffled")
                        }
                    }
                }
            }
            .padding(28)
        }
        .onGeometryChange(for: Double.self) { $0.size.width } action: { width = $0 }
        .task(id: store.libraryVersion) {
            spots = await store.load { try $0.genreMap() } ?? []
            // For tests: `--play-genre <name>` plays that genre as a click does.
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "--play-genre"), args.indices.contains(i + 1) { play(args[i + 1]) }
        }
    }

    /// The line under the title, as on Windows.
    private func summary(_ unplaced: Bool) -> String {
        guard let spots else { return "Loading…" }
        if spots.isEmpty { return "Your songs have no genres in their tags yet." }
        if unplaced {
            return "Needle measures each song's sound in the background (Settings › Library). Genres take their place as their songs are measured."
        }
        return "\(count(spots.count, "genre")), each where its songs sound. Click one to play it."
    }

    private func map(_ bubbles: [GenreBubble], width: Double, height: Double) -> some View {
        let colors = store.colors
        return ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 14, style: .continuous).fill(colors.raised)
                .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(colors.ink3.opacity(0.2)))
            ForEach(bubbles) { bubble in
                let tone = GenreLayout.tone(bubble.genre, dark: colors.dark)
                let many = bubble.genre.contains(" ") || bubble.genre.contains("-")
                Button { play(bubble.genre) } label: {
                    VStack(spacing: 1) {
                        Text(bubble.genre)
                            .font(.system(size: GenreLayout.label(bubble.genre, size: bubble.size), weight: .semibold))
                            .foregroundStyle(colors.ink)
                            .lineLimit(many ? 2 : 1)
                            .minimumScaleFactor(0.8)
                        if GenreLayout.roomy(bubble) {
                            Text(count(bubble.songs, "song")).font(.needle(.caption)).foregroundStyle(colors.ink3)
                        }
                    }
                    .multilineTextAlignment(.center)
                    .frame(width: bubble.size - 10)
                    .frame(width: bubble.size, height: bubble.size)
                    .background(Circle().fill(tone.opacity(hovered == bubble.genre ? 0.4 : 0.22)))
                    .overlay(Circle().strokeBorder(tone.opacity(0.65), lineWidth: 1))
                    .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .onHover { inside in hovered = inside ? bubble.genre : (hovered == bubble.genre ? nil : hovered) }
                .help("\(bubble.genre): \(count(bubble.songs, "song")) · click to play, shuffled")
                .accessibilityLabel("\(bubble.genre), \(count(bubble.songs, "song"))")
                .accessibilityHint("Plays it, shuffled")
                .position(x: bubble.x, y: bubble.y)
            }
            // The axes' words, in the corners as on Windows.
            Group {
                Text("Lively").padding(.leading, 12).padding(.top, 8)
                Text("Calm").padding(.leading, 12).padding(.bottom, 8).frame(maxHeight: .infinity, alignment: .bottomLeading)
                Text("Dark").padding(.leading, 70).padding(.bottom, 8).frame(maxHeight: .infinity, alignment: .bottomLeading)
                Text("Bright").padding(.trailing, 12).padding(.bottom, 8).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
            }
            .font(.needle(.caption)).foregroundStyle(colors.ink3)
            .allowsHitTesting(false)
        }
        .frame(width: width, height: height)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private func play(_ genre: String) {
        let (rule, reason) = ("genre = \(Rule.quote(genre))", "From the genre map")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: true, reason: reason) } }
    }
}
