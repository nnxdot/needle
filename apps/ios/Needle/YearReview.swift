import SwiftUI

/// "Your 2026 in music", a card at a time: the hours and the year by month, the top artist, and
/// the top songs; then Save as a picture, Make a playlist, and Play top 50. From the core's
/// `year_in_review`.
struct YearReviewView: View {
    @Environment(Store.self) private var store
    @State private var years: [Int32] = []
    @State private var year: Int32?
    @State private var review: YearInReview?
    @State private var loading = true
    /// The covers and the top artist's photo, read before they are drawn, so the picture has them.
    @State private var images: [String: UIImage] = [:]
    @State private var picture: Image?
    /// The card shown (for tests, `-year-page 1|2` opens on that one).
    @State private var page = CommandLine.arguments.firstIndex(of: "-year-page").flatMap { i in
        CommandLine.arguments.indices.contains(i + 1) ? Int(CommandLine.arguments[i + 1]) : nil } ?? 0

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                if years.count > 1 {
                    Picker("Year", selection: $year) {
                        ForEach(years, id: \.self) { Text(String($0)).tag(Int32?.some($0)) }
                    }
                    .pickerStyle(.segmented)
                }
                if let review, review.listens > 0 {
                    TabView(selection: $page) {
                        ForEach(0..<3, id: \.self) { page in
                            YearCard(review: review, page: page, images: images)
                                .padding(.horizontal, 2)
                                .tag(page)
                        }
                    }
                    .tabViewStyle(.page(indexDisplayMode: .always))
                    .frame(height: 470)
                    buttons(review)
                } else if !loading {
                    ContentUnavailableView("No listening in \(String(year ?? store.reviewYear)) yet", systemImage: "sparkles",
                                           description: Text("Play some music and come back."))
                        .padding(.top, 60)
                }
            }
            .padding()
        }
        .navigationTitle("Your year in music")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .overlay { if loading { ProgressView() } }
        .task {
            years = await store.load { try $0.listeningYears() } ?? []
            if year == nil { year = store.reviewYear }
        }
        .task(id: year) {
            guard let y = year else { return }
            loading = true
            picture = nil
            review = await store.load { try $0.yearInReview(year: y) }
            loading = false
            images = await Self.images(for: review, store: store)
            if let review { picture = render(review) }
        }
    }

    private func buttons(_ review: YearInReview) -> some View {
        VStack(spacing: 10) {
            Button {
                store.play(review.topSongs.prefix(50).map(\.song))
            } label: {
                Label("Play top 50", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
            }
            .buttonStyle(.glassProminent)
            .foregroundStyle(store.colors.onAccent)
            HStack(spacing: 10) {
                if let picture {
                    ShareLink(item: picture, preview: SharePreview("Your \(String(review.year)) in music", image: picture)) {
                        Label("Save as a picture", systemImage: "square.and.arrow.up").frame(maxWidth: .infinity).padding(.vertical, 4)
                    }
                    .buttonStyle(.glass)
                } else {
                    Button {} label: { Label("Save as a picture", systemImage: "square.and.arrow.up").frame(maxWidth: .infinity).padding(.vertical, 4) }
                        .buttonStyle(.glass)
                        .disabled(true)
                }
                Button { makePlaylist(review.year) } label: {
                    Label("Make a playlist", systemImage: "music.note.list").frame(maxWidth: .infinity).padding(.vertical, 4)
                }
                .buttonStyle(.glass)
            }
            .font(.needle(.subheadline).weight(.semibold))
        }
    }

    static func images(for review: YearInReview?, store: Store) async -> [String: UIImage] {
        guard let review else { return [:] }
        var found: [String: UIImage] = [:]
        for top in review.topSongs.prefix(YearCard.shown) {
            if let path = top.song.artwork, found[path] == nil, let image = await Covers.load(path, size: 160) { found[path] = image }
        }
        if let name = review.topArtist?.name,
           let path = await store.load({ $0.artistPhoto(name: name) }) ?? nil,
           let image = await Covers.load(path, size: 500) {
            found[YearCard.artistKey] = image
        }
        return found
    }

    /// The three cards in a row, as one picture to keep or send.
    private func render(_ review: YearInReview) -> Image? {
        let cards = HStack(spacing: 24) {
            ForEach(0..<3, id: \.self) { YearCard(review: review, page: $0, images: images).frame(width: 360, height: 470) }
        }
        .padding(32)
        .background(Color(red: 0.08, green: 0.06, blue: 0.07))
        .environment(store)
        .environment(\.colorScheme, .dark)
        let renderer = ImageRenderer(content: cards)
        renderer.scale = 3
        return renderer.uiImage.map { Image(uiImage: $0) }
    }

    private func makePlaylist(_ year: Int32) {
        Task {
            if await store.ask({ try $0.saveYearPlaylist(year: year) }) != nil {
                store.message = "Saved “Top songs of \(String(year))”"
                store.libraryChanged()
            }
        }
    }
}

/// One of the year's cards, on a warm gradient as in the phone mockup.
struct YearCard: View {
    let review: YearInReview
    /// 0: the hours, 1: the top artist, 2: the top songs.
    let page: Int
    let images: [String: UIImage]

    static let shown = 5
    static let artistKey = "artist"

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("YOUR \(String(review.year)) IN MUSIC")
                .font(.system(size: 11, design: .monospaced)).kerning(1.6)
                .foregroundStyle(ThemeColors.amber)
                .padding(.bottom, 18)
            switch page {
            case 0: hours
            case 1: artist
            default: songs
            }
            Spacer(minLength: 0)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .foregroundStyle(.white)
        .background {
            LinearGradient(colors: [Color(red: 0.42, green: 0.26, blue: 0.13), Color(red: 0.26, green: 0.17, blue: 0.2), Color(red: 0.2, green: 0.14, blue: 0.3)],
                           startPoint: .top, endPoint: .bottom)
                .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
        }
    }

    private var hours: some View {
        let hours = Int(review.seconds / 3600)
        let days = Int(review.seconds / 86400)
        let most = max(review.months.map(\.seconds).max() ?? 0, 1)
        var line: [String] = []
        if days > 0 { line.append("That is \(count(days, "whole day", "whole days")).") }
        line.append(count(Int(review.plays), "play") + (review.newSongs > 0 ? ", \(Int(review.newSongs).formatted()) new songs found." : "."))
        return VStack(alignment: .leading, spacing: 4) {
            Text(hours > 0 ? hours.formatted() : Int((review.seconds / 60).rounded()).formatted())
                .font(.system(size: 72, design: .serif))
                .foregroundStyle(ThemeColors.amber)
                .lineLimit(1).minimumScaleFactor(0.5)
            Text(hours > 0 ? "hours of music" : "minutes of music").font(.system(size: 18, weight: .bold))
            Text(line.joined(separator: " "))
                .font(.system(size: 14)).foregroundStyle(.white.opacity(0.7))
                .fixedSize(horizontal: false, vertical: true)
            Text("By month").font(.system(size: 13)).foregroundStyle(.white.opacity(0.7)).padding(.top, 22).padding(.bottom, 8)
            HStack(alignment: .bottom, spacing: 5) {
                ForEach(review.months, id: \.month) { m in
                    RoundedRectangle(cornerRadius: 2)
                        .fill(m.seconds >= most ? ThemeColors.amber : ThemeColors.amber.opacity(0.4))
                        .frame(height: max(3, 120 * m.seconds / most))
                        .frame(maxWidth: .infinity)
                }
            }
            .frame(height: 120, alignment: .bottom)
            .accessibilityElement()
            .accessibilityLabel("Listening by month")
        }
    }

    private var artist: some View {
        VStack(spacing: 12) {
            Text("Top artist").font(.system(size: 13)).foregroundStyle(.white.opacity(0.7))
            if let top = review.topArtist {
                ZStack {
                    if let image = images[Self.artistKey] {
                        Image(uiImage: image).resizable().scaledToFill()
                    } else {
                        let p = Palette(top.name)
                        LinearGradient(colors: [p.colors[0], p.colors[1].opacity(0.7)], startPoint: .topLeading, endPoint: .bottomTrailing)
                    }
                }
                .frame(width: 170, height: 170)
                .clipShape(Circle())
                .padding(.vertical, 12)
                Text(top.name).font(.system(size: 28, weight: .bold)).multilineTextAlignment(.center).lineLimit(2)
                Text(artistLine(top)).font(.system(size: 15)).foregroundStyle(.white.opacity(0.7)).multilineTextAlignment(.center)
            } else {
                Text("Nobody yet").foregroundStyle(.white.opacity(0.7))
            }
        }
        .frame(maxWidth: .infinity)
    }

    /// "212 hours · 1,903 plays", then the months in the top five.
    private func artistLine(_ top: YearTopArtist) -> String {
        let hours = Int(top.seconds / 3600)
        let time = hours > 0 ? count(hours, "hour") : count(Int((top.seconds / 60).rounded()), "minute")
        let months = Int(top.monthsInTopFive)
        let place = months >= 12 ? "\nIn your top 5 every month" : months > 1 ? "\nIn your top 5 for \(months) months" : ""
        return "\(time) · \(count(Int(top.plays), "play"))\(place)"
    }

    private var songs: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Top songs").font(.system(size: 13)).foregroundStyle(.white.opacity(0.7)).padding(.bottom, 6)
            ForEach(Array(review.topSongs.prefix(Self.shown).enumerated()), id: \.offset) { i, top in
                HStack(spacing: 12) {
                    Text("\(i + 1)").font(.system(size: 14)).foregroundStyle(ThemeColors.amber).frame(width: 16)
                    ZStack {
                        if let path = top.song.artwork, let image = images[path] {
                            Image(uiImage: image).resizable().scaledToFill()
                        } else {
                            let p = Palette(top.song.album + top.song.artist)
                            LinearGradient(colors: [p.colors[0], p.colors[1]], startPoint: .topLeading, endPoint: .bottomTrailing)
                        }
                    }
                    .frame(width: 44, height: 44)
                    .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(top.song.title).font(.system(size: 15, weight: .medium)).lineLimit(1)
                        Text(top.song.artist).font(.system(size: 13)).foregroundStyle(.white.opacity(0.7)).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    Text(Int(top.plays).formatted()).font(.system(size: 14)).foregroundStyle(.white.opacity(0.7))
                }
                .padding(.vertical, 10)
                .accessibilityElement(children: .combine)
                if i < min(review.topSongs.count, Self.shown) - 1 { Divider().overlay(.white.opacity(0.1)) }
            }
        }
    }
}

/// Home's "Your year in music" card, in December.
struct YearHomeCard: View {
    let year: Int32

    var body: some View {
        NavigationLink(value: Route.wrapped) {
            HStack(spacing: 14) {
                Image(systemName: "sparkles").font(.system(size: 24)).foregroundStyle(ThemeColors.amber)
                VStack(alignment: .leading, spacing: 3) {
                    Text("YOUR \(String(year)) IN MUSIC").font(.system(size: 11, design: .monospaced)).kerning(1.4).foregroundStyle(ThemeColors.amber)
                    Text("Your year in music").font(.needle(.headline)).foregroundStyle(.white)
                    Text("Your hours, your top artist, and your top songs.").font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.7))
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right").foregroundStyle(.white.opacity(0.6))
            }
            .padding(18)
            .background {
                LinearGradient(colors: [Color(red: 0.42, green: 0.26, blue: 0.13), Color(red: 0.2, green: 0.14, blue: 0.3)], startPoint: .leading, endPoint: .trailing)
                    .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
            }
        }
        .buttonStyle(.plain)
    }
}
