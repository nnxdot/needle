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
    /// The height of the corner words' strips (Lively at the top; Calm, Dark, Bright at the foot).
    static let wordRoom = 22.0

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
        // Nudged apart where they overlap, to a 4 pt gap between edges, and kept inside the map,
        // clear of the strips at its top and foot where the corner words sit.
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
                bubbles[i].y = min(max(bubbles[i].y, r + 4 + wordRoom), height - r - 4 - wordRoom)
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
    static let smallest = 9.5, largest = 17.0

    /// How wide the longest word of a name is (a hyphen may end a line), in points, at `font` size.
    static func widest(_ genre: String, font: Double) -> Double {
        let face = UIFont.systemFont(ofSize: font, weight: .semibold)
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
/// sound (dark to bright across, calm to lively up); a tap plays it shuffled. The genres past
/// the 40 largest, and those with no song measured yet, are chips under it.
struct GenreMapView: View {
    @Environment(Store.self) private var store
    @State private var spots: [GenreSpot]?
    /// "map" (where the songs sound), "cloud", or "bubbles" (by family).
    @AppStorage("genre-view") private var view = "map"
    /// Cloud and bubbles by plays instead of songs.
    @AppStorage("genre-by-plays") private var byPlays = false
    @State private var art: GenreArt?
    /// The view and order `art` was laid out for ("cloud|false").
    @State private var artFor = ""
    /// Why the cloud or bubbles could not be laid out, and for which view and order.
    @State private var artProblem: (String, String)?
    /// Goes up with Try Again.
    @State private var tries = 0

    var body: some View {
        GeometryReader { geo in
            // Upright on a phone: as wide as the screen and taller than wide; the bubbles smaller
            // by the map's area against the widest computer map (980 by 600).
            let side = min(geo.size.width - 40, 640)
            let height = min(max(side * 1.6, 300), 760)
            let laid = GenreLayout.lay(spots ?? [], width: side, height: height,
                                       scale: min(1, max(0.6, (side * height / (980 * 600)).squareRoot())))
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let spots, spots.isEmpty {
                        ContentUnavailableView("No genres yet", systemImage: "circle.hexagongrid",
                                               description: Text("Your songs have no genres in their tags yet."))
                    } else if let spots {
                        Picker("View", selection: $view) {
                            Text("Map").tag("map")
                            Text("Cloud").tag("cloud")
                            Text("Bubbles").tag("bubbles")
                        }
                        .pickerStyle(.segmented)
                        if view != "map" {
                            Picker("Size by", selection: $byPlays) {
                                Text("By songs").tag(false)
                                Text("By plays").tag(true)
                            }
                            .pickerStyle(.segmented)
                            if let artProblem, artProblem.0 == "\(view)|\(byPlays)" {
                                LoadProblem(title: view == "cloud" ? "Could not make the cloud" : "Could not make the bubbles",
                                            words: artProblem.1) { tries += 1 }
                            } else if let art, artFor == "\(view)|\(byPlays)" {
                                Text(art.items.isEmpty ? (byPlays ? "Nothing played yet." : "No genres yet.")
                                     : "\(count(art.items.count, "genre")). \(view == "cloud" ? "As big as" : "Grouped by family, as big as") \(byPlays ? "their plays" : "their songs"). Tap one to play it.")
                                    .font(.needle(.subheadline)).foregroundStyle(store.colors.muted)
                                if !art.items.isEmpty {
                                    GenreArtView(art: art, cloud: view == "cloud", width: side, maxHeight: max(geo.size.height - 200, 320), dark: store.colors.dark) { play($0) }
                                }
                            }
                        } else {
                        Text(laid.bubbles.isEmpty
                             ? "Needle measures each song's sound in the background (Settings › Library). Genres take their place as their songs are measured."
                             : "\(count(spots.count, "genre")), each where its songs sound. Tap one to play it.")
                            .font(.needle(.subheadline)).foregroundStyle(store.colors.muted)
                        if !laid.bubbles.isEmpty {
                            map(laid.bubbles, width: side, height: height)
                        }
                        if !laid.chips.isEmpty {
                            Text("More genres").font(.needle(.title3).weight(.semibold))
                            FlowLayout {
                                ForEach(laid.chips, id: \.genre) { chip in
                                    Button { play(chip.genre) } label: {
                                        HStack(spacing: 8) {
                                            Text(chip.genre)
                                            Text(Int(chip.songs).formatted()).foregroundStyle(store.colors.muted)
                                        }
                                        .font(.needle(.subheadline))
                                    }
                                    .buttonStyle(.glass)
                                }
                            }
                        }
                        }
                    }
                }
                .padding(20)
            }
        }
        // A readable width in a wide window (an iPad), not a band across it.
        .readableWidth(860)
        .navigationTitle("Genre map")
        .themed()
        .task(id: "\(store.libraryVersion)|\(view)|\(byPlays)|\(tries)") {
            guard view != "map" else { return }
            let (kind, plays) = (view, byPlays)
            // Upright on a phone: a little taller than wide.
            let result = await store.attempt { try $0.genreArtJson(kind: kind, byPlays: plays, aspect: 0.8) }
            // Switched to another view or order meanwhile: that one has its own load.
            guard !Task.isCancelled, kind == view, plays == byPlays else { return }
            switch result {
            case .success(let json):
                art = try? JSONDecoder().decode(GenreArt.self, from: Data(json.utf8))
                artProblem = art == nil ? ("\(kind)|\(plays)", "Needle could not read its own answer.") : nil
            case .failure(let error):
                art = nil
                artProblem = ("\(kind)|\(plays)", error.words)
            }
            artFor = "\(kind)|\(plays)"
        }
        .task(id: store.libraryVersion) {
            spots = await store.load { try $0.genreMap() } ?? []
            // For tests: `-play-genre <name>` plays that genre as a tap does.
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "-play-genre"), args.indices.contains(i + 1) { play(args[i + 1]) }
        }
    }

    private func map(_ bubbles: [GenreBubble], width: Double, height: Double) -> some View {
        let colors = store.colors
        return ZStack(alignment: .topLeading) {
            RoundedRectangle.soft(Corner.large).fill(colors.card)
            ForEach(bubbles) { bubble in
                let tone = GenreLayout.tone(bubble.genre, dark: colors.dark)
                let many = bubble.genre.contains(" ") || bubble.genre.contains("-")
                Button { play(bubble.genre) } label: {
                    VStack(spacing: 1) {
                        Text(bubble.genre)
                            .font(.system(size: GenreLayout.label(bubble.genre, size: bubble.size), weight: .semibold))
                            .foregroundStyle(colors.text)
                            .lineLimit(many ? 2 : 1)
                            .minimumScaleFactor(0.8)
                        if GenreLayout.roomy(bubble) {
                            Text(count(bubble.songs, "song")).font(.needle(.caption)).foregroundStyle(colors.muted)
                        }
                    }
                    .multilineTextAlignment(.center)
                    .frame(width: bubble.size - 8)
                    .frame(width: bubble.size, height: bubble.size)
                    .background(Circle().fill(tone.opacity(0.22)))
                    .overlay(Circle().strokeBorder(tone.opacity(0.65), lineWidth: 1))
                    .contentShape(.circle)
                }
                .buttonStyle(Pressable())
                .accessibilityLabel("\(bubble.genre), \(count(bubble.songs, "song"))")
                .accessibilityHint("Plays it, shuffled")
                .position(x: bubble.x, y: bubble.y)
            }
            // The axes' words, in the corners as on Windows.
            Group {
                Text("Lively").padding(.leading, 12).padding(.top, 8)
                Text("Calm").padding(.leading, 12).padding(.bottom, 8).frame(maxHeight: .infinity, alignment: .bottomLeading)
                Text("Dark").padding(.leading, 60).padding(.bottom, 8).frame(maxHeight: .infinity, alignment: .bottomLeading)
                Text("Bright").padding(.trailing, 12).padding(.bottom, 8).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
            }
            .font(.needle(.caption)).foregroundStyle(colors.muted)
            .allowsHitTesting(false)
        }
        .frame(width: width, height: height)
        .clipShape(.soft(Corner.large))
    }

    private func play(_ item: GenreArt.Item) {
        Task {
            let spellings = item.spellings
            let songs = await store.ask { try $0.genresSongs(spellings: spellings) } ?? []
            if songs.isEmpty { return }
            store.play(songs)
            store.message = "Playing \(item.genre), shuffled"
        }
    }

    private func play(_ genre: String) {
        Task {
            let songs = await store.ask { try $0.genreSongs(name: genre) } ?? []
            if songs.isEmpty { return }
            store.play(songs.shuffled())
            store.message = "Playing \(genre), shuffled"
        }
    }
}

/// The core's cloud or bubbles (see genre_art.rs): places and sizes in a box 1 wide and `height`
/// high.
struct GenreArt: Decodable {
    struct Item: Decodable, Identifiable {
        var genre: String
        var spellings: [String]
        var songs: Int
        var plays: Int
        var family: String
        var hue: Double
        var x: Double
        var y: Double
        var size: Double
        /// A circle's name fits at `fit` × the radius (its longest word across 80 % of it).
        var fit: Double
        var id: String { genre }
    }
    var height: Double
    var items: [Item]
}

/// The genre cloud (words) or bubbles (circles by family), drawn where the core placed them,
/// scaled to `width` (or to `maxHeight`, when the picture is taller). A tap plays a genre.
struct GenreArtView: View {
    let art: GenreArt
    let cloud: Bool
    let width: Double
    let maxHeight: Double
    let dark: Bool
    let play: (GenreArt.Item) -> Void

    var body: some View {
        let scale = min(width, maxHeight / max(art.height, 0.01))
        ZStack(alignment: .topLeading) {
            ForEach(art.items) { item in
                Button { play(item) } label: { label(item, scale: scale) }
                    .buttonStyle(Pressable())
                    .accessibilityLabel("\(item.genre), \(count(item.songs, "song"))")
                    .accessibilityHint("Plays it, shuffled")
                    .position(x: item.x * scale, y: item.y * scale)
            }
        }
        .frame(width: scale, height: art.height * scale, alignment: .topLeading)
        .frame(maxWidth: .infinity, alignment: .center)
    }

    private func tone(_ item: GenreArt.Item) -> Color {
        Color(hue: item.hue / 360, saturation: dark ? 0.5 : 0.65, brightness: dark ? 0.85 : 0.55)
    }

    @ViewBuilder
    private func label(_ item: GenreArt.Item, scale: Double) -> some View {
        if cloud {
            Text(item.genre)
                .font(.system(size: max(item.size * scale, 6), weight: .bold))
                .foregroundStyle(tone(item))
                .fixedSize()
        } else {
            let r = item.size * scale
            ZStack {
                Circle().fill(tone(item).opacity(0.9))
                // As big as fits, its longest word on one line; no name when too small.
                let font = min(min(max(r * 0.32, 8), 20), r * item.fit)
                if font >= 7 {
                    Text(item.genre)
                        .font(.system(size: font, weight: .semibold))
                        .foregroundStyle(dark ? Color.black.opacity(0.8) : Color.white)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                        .minimumScaleFactor(0.6)
                        .frame(width: r * 1.6)
                }
            }
            .frame(width: r * 2, height: r * 2)
            .contentShape(.circle)
        }
    }
}
