import SwiftUI

/// A genre's bubble on the map: where it sits (0 to 1) and how wide it is (points).
struct GenreBubble: Identifiable {
    let genre: String
    let songs: Int
    var x: Double
    var y: Double
    let size: Double
    var id: String { genre }
}

/// The genre map's layout: the 40 largest genres with a place, sized by their songs, nudged
/// apart so names do not sit on one another; the rest (and those with no place yet) as chips.
enum GenreLayout {
    /// The genres to draw on a map `side` points wide, and the ones left for the chips.
    static func lay(_ spots: [(genre: String, songs: Int, placed: Bool, x: Double, y: Double)], side: Double)
        -> (bubbles: [GenreBubble], chips: [(genre: String, songs: Int)]) {
        let placed = spots.filter(\.placed).sorted { $0.songs > $1.songs }
        let shown = placed.prefix(40)
        let most = Double(shown.first?.songs ?? 1)
        var bubbles = shown.map { spot in
            // Area follows the songs: the largest about a quarter of the map across; none too
            // small for its longest word at the smallest size.
            let size = max(widest(spot.genre, font: smallest) + 12, side * 0.24 * (Double(spot.songs) / most).squareRoot())
            return GenreBubble(genre: spot.genre, songs: spot.songs, x: spot.x, y: 1 - spot.y, size: size)
        }
        // A few rounds of pushing overlapping bubbles apart, kept inside the map.
        for _ in 0..<60 {
            var moved = false
            for i in bubbles.indices {
                for j in bubbles.indices where j > i {
                    let dx = (bubbles[j].x - bubbles[i].x) * side, dy = (bubbles[j].y - bubbles[i].y) * side
                    let distance = max((dx * dx + dy * dy).squareRoot(), 0.01)
                    let need = (bubbles[i].size + bubbles[j].size) / 2 * 0.92
                    guard distance < need else { continue }
                    let push = (need - distance) / 2 / side
                    let (ux, uy) = distance > 0.01 ? (dx / distance, dy / distance) : (1.0, 0.0)
                    bubbles[i].x -= ux * push; bubbles[i].y -= uy * push
                    bubbles[j].x += ux * push; bubbles[j].y += uy * push
                    moved = true
                }
            }
            for i in bubbles.indices {
                let r = bubbles[i].size / 2 / side
                bubbles[i].x = min(max(bubbles[i].x, r), 1 - r)
                bubbles[i].y = min(max(bubbles[i].y, r), 1 - r)
            }
            if !moved { break }
        }
        let onMap = Set(bubbles.map(\.genre))
        let chips = spots.filter { !onMap.contains($0.genre) }.sorted { $0.songs > $1.songs }.map { ($0.genre, $0.songs) }
        return (bubbles, chips)
    }

    /// The smallest and largest name sizes.
    static let smallest = 11.0, largest = 17.0

    /// How wide the longest word of a name is (a hyphen may end a line), in points, at `font` size.
    static func widest(_ genre: String, font: Double) -> Double {
        let face = NSFont.systemFont(ofSize: font, weight: .semibold)
        return genre.replacingOccurrences(of: "-", with: "- ").split(separator: " ").map { Double((String($0) as NSString).size(withAttributes: [.font: face]).width) }.max() ?? 0
    }

    /// A bubble's name size: as large as the bubble allows, with its longest word on one line.
    static func label(_ genre: String, size: Double) -> Double {
        let fit = smallest * (size - 12) / max(widest(genre, font: smallest), 1)
        return max(smallest, min(largest, size * 0.16, fit))
    }

    /// A bubble's colour by its place: dark and calm deep blue, bright and lively warm.
    static func hue(x: Double, y: Double) -> (hue: Double, saturation: Double, brightness: Double) {
        (0.62 - 0.55 * x, 0.5 + 0.3 * (1 - y), 0.5 + 0.4 * x)
    }
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

/// The genre map: a bubble for each genre, as big as its number of songs, at the middle of
/// its songs' sound (dark to bright across, calm to lively up); a click plays it shuffled. The
/// genres past the 40 largest, and those with no song measured yet, are chips under it.
struct GenreMapPage: View {
    private var store: Store { .shared }
    @State private var spots: [GenreSpot]?
    @State private var hovered: String?
    private let side: Double = 600

    var body: some View {
        let colors = store.colors
        let laid = GenreLayout.lay((spots ?? []).map { ($0.genre, Int($0.songs), $0.placed, Double($0.x), Double($0.y)) }, side: side)
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Genre map").font(store.display(28))
                    Text(spots.map { $0.isEmpty ? "No genres yet" : "\(count($0.count, "genre")) · click one to play it" } ?? "Loading…")
                        .font(.needle(.callout)).foregroundStyle(colors.ink2)
                }
                if let spots, spots.isEmpty {
                    Text("Needle reads genres from your songs' tags. Songs with a genre show here.")
                        .foregroundStyle(colors.ink2)
                } else if spots != nil {
                    if laid.bubbles.isEmpty {
                        Text("The map fills in as Needle measures each song's sound (Settings › Library › Measure songs for radio).")
                            .foregroundStyle(colors.ink2)
                    } else {
                        map(laid.bubbles).frame(maxWidth: .infinity)
                    }
                    if !laid.chips.isEmpty {
                        FlowLayout {
                            ForEach(laid.chips, id: \.genre) { chip in
                                Button { play(chip.genre) } label: {
                                    Text("\(chip.genre)  \(chip.songs.formatted())")
                                        .font(.needle(.callout))
                                        .padding(.horizontal, 12).padding(.vertical, 6)
                                        .background(colors.raised, in: Capsule())
                                }
                                .buttonStyle(.plain)
                                .help("Play \(chip.genre), shuffled")
                            }
                        }
                    }
                }
            }
            .padding(28)
        }
        .task(id: store.libraryVersion) {
            spots = await store.load { try $0.genreMap() } ?? []
            // For tests: `--play-genre <name>` plays that genre as a click does.
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "--play-genre"), args.indices.contains(i + 1) { play(args[i + 1]) }
        }
    }

    private func map(_ bubbles: [GenreBubble]) -> some View {
        let colors = store.colors
        return ZStack {
            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(colors.raised.opacity(0.5))
            ForEach(bubbles) { bubble in
                let tone = GenreLayout.hue(x: bubble.x, y: bubble.y)
                Button { play(bubble.genre) } label: {
                    Text(bubble.genre)
                        .font(.system(size: GenreLayout.label(bubble.genre, size: bubble.size), weight: .semibold))
                        .multilineTextAlignment(.center)
                        .lineLimit(3)
                        .foregroundStyle(.white)
                        .frame(width: bubble.size - 8)
                        .frame(width: bubble.size, height: bubble.size)
                        .background(Circle().fill(Color(hue: tone.hue, saturation: tone.saturation, brightness: tone.brightness).opacity(0.85)))
                        .overlay(Circle().strokeBorder(.white.opacity(hovered == bubble.genre ? 0.7 : 0.15), lineWidth: 1.5))
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .scaleEffect(hovered == bubble.genre && store.motion ? 1.06 : 1)
                .onHover { inside in withAnimation(store.motion ? .snappy(duration: 0.15) : nil) { hovered = inside ? bubble.genre : nil } }
                .help("\(bubble.genre): \(count(bubble.songs, "song")) · click to play, shuffled")
                .accessibilityLabel("\(bubble.genre), \(count(bubble.songs, "song"))")
                .accessibilityHint("Plays it, shuffled")
                .position(x: bubble.x * side, y: bubble.y * side)
            }
            // The axes' words.
            VStack { Text("Lively"); Spacer(); Text("Calm") }
                .font(.needle(.caption).weight(.semibold)).foregroundStyle(colors.ink3)
                .padding(.vertical, -22)
                .allowsHitTesting(false)
            HStack { Text("Dark").rotationEffect(.degrees(-90)).fixedSize(); Spacer(); Text("Bright").rotationEffect(.degrees(90)).fixedSize() }
                .font(.needle(.caption).weight(.semibold)).foregroundStyle(colors.ink3)
                .padding(.horizontal, -30)
                .allowsHitTesting(false)
        }
        .frame(width: side, height: side)
        .padding(.vertical, 22)
    }

    private func play(_ genre: String) {
        let (rule, reason) = ("genre = \(Rule.quote(genre))", "From the genre \(genre)")
        Task { _ = await store.ask { try $0.playView(query: rule, playlist: nil, filter: "", start: 0, shuffle: true, reason: reason) } }
    }
}
