import SwiftUI

/// "Your 2026 in music": three cards (the hours and the year by month, the top artist, and the
/// top songs), then Save as a picture, Make a playlist, and Play top 50. From the core's
/// `year_in_review`.
struct YearReviewPage: View {
    private var store: Store { .shared }
    let year: Int
    @State private var years: [Int32] = []
    @State private var review: YearInReview?
    @State private var loading = true
    /// The covers and the top artist's photo, read before they are drawn, so a picture of the
    /// cards has them too.
    @State private var images: [String: NSImage] = [:]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                if years.count > 1 {
                    HStack {
                        Spacer()
                        Picker("Year", selection: Binding(get: { Int32(year) }, set: { store.navigate(.wrapped(Int($0))) })) {
                            ForEach(years, id: \.self) { Text(String($0)).tag($0) }
                        }
                        .fixedSize()
                    }
                }
                if loading {
                    HStack { ProgressView().controlSize(.small); Text("Gathering your year…").foregroundStyle(.secondary) }
                } else if let review, review.listens > 0 {
                    YearCards(review: review, images: images, actions: YearActions(
                        picture: { savePicture(review) }, playlist: { makePlaylist() }, play: { play(review) }))
                } else {
                    Text("No listening in \(String(year)) yet. Play some music and come back.").foregroundStyle(.secondary)
                }
            }
            .padding(28)
        }
        .task(id: year) {
            loading = true
            years = await store.load { try $0.listeningYears() } ?? []
            let y = Int32(year)
            review = await store.load { try $0.yearInReview(year: y) }
            loading = false
            images = await Self.images(for: review, store: store)
            // For tests: `--year-save` saves the picture and makes the playlist, as the buttons do.
            if let review, CommandLine.arguments.contains("--year-save") { savePicture(review); makePlaylist() }
        }
    }

    static func images(for review: YearInReview?, store: Store) async -> [String: NSImage] {
        guard let review else { return [:] }
        var found: [String: NSImage] = [:]
        for top in review.topSongs.prefix(YearCards.shown) {
            if let path = top.song.artwork, found[path] == nil, let image = await Covers.load(path, size: 160) { found[path] = image }
        }
        if let name = review.topArtist?.name,
           let path = await Photos.find(name, online: false, store: store),
           let image = await Covers.load(path, size: 400) {
            found[YearCards.artistKey] = image
        }
        return found
    }

    /// The cards as a picture, in Pictures › Needle (as lyric cards on Windows).
    private func savePicture(_ review: YearInReview) {
        let renderer = ImageRenderer(content: YearCards(review: review, images: images, actions: nil)
            .frame(width: 1240)
            .environment(\.colorScheme, .dark))
        renderer.scale = 2
        guard let picture = renderer.nsImage, let tiff = picture.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
            store.fail("Could not make the picture.")
            return
        }
        let folder = FileManager.default.urls(for: .picturesDirectory, in: .userDomainMask)[0].appendingPathComponent("Needle")
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            var url = folder.appendingPathComponent("Your \(review.year) in music.png")
            var n = 2
            while FileManager.default.fileExists(atPath: url.path) {
                url = folder.appendingPathComponent("Your \(review.year) in music \(n).png")
                n += 1
            }
            try png.write(to: url)
            store.notify("Saved to Pictures › Needle.")
        } catch {
            store.fail("Could not save the picture: \(error.localizedDescription)")
        }
    }

    private func makePlaylist() {
        let y = Int32(year)
        Task {
            if await store.ask({ try $0.saveYearPlaylist(year: y) }) != nil {
                store.notify("Saved “Top songs of \(String(year))”.")
                store.libraryChanged()
            }
        }
    }

    private func play(_ review: YearInReview) {
        let ids = review.topSongs.prefix(50).map(\.song.id)
        let reason = "One of your top songs of \(review.year)"
        store.run { _ = try $0.playSongs(ids: Array(ids), start: 0, reason: reason) }
    }
}

/// What the year's buttons do (none in a picture).
struct YearActions {
    let picture: () -> Void
    let playlist: () -> Void
    let play: () -> Void
}

/// The year's three cards on their warm panel, as in the mockup.
struct YearCards: View {
    private var store: Store { .shared }
    let review: YearInReview
    let images: [String: NSImage]
    let actions: YearActions?

    /// How many top songs the card lists.
    static let shown = 4
    static let artistKey = "artist"
    private static let amber = Color(red: 0.886, green: 0.706, blue: 0.424)

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("YOUR \(String(review.year)) IN MUSIC")
                .font(.system(size: 11, design: .monospaced)).kerning(1.6)
                .foregroundStyle(Self.amber)
            HStack(alignment: .top, spacing: 20) {
                hours.frame(width: 300)
                artist.frame(width: 300)
                songs.frame(maxWidth: .infinity)
            }
            .fixedSize(horizontal: false, vertical: true)
        }
        .padding(36)
        .foregroundStyle(.white)
        .background {
            LinearGradient(colors: [Color(red: 0.16, green: 0.11, blue: 0.2), Color(red: 0.2, green: 0.14, blue: 0.13), Color(red: 0.33, green: 0.2, blue: 0.11)],
                           startPoint: .topLeading, endPoint: .bottomTrailing)
                .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        }
    }

    private func card<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 0) { content() }
            .padding(20)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(.white.opacity(0.06), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private var hours: some View {
        let hours = Int(review.seconds / 3600)
        let days = Int(review.seconds / 86400)
        let detail = ([days > 0 ? count(days, "day") : nil, count(Int(review.plays), "play"),
                       review.newSongs > 0 ? "\(Int(review.newSongs).formatted()) new songs" : nil] as [String?])
            .compactMap { $0 }.joined(separator: " · ")
        let most = max(review.months.map(\.seconds).max() ?? 0, 1)
        return card {
            Text(hours > 0 ? hours.formatted() : Int((review.seconds / 60).rounded()).formatted())
                .font(.system(size: 62, design: .serif))
                .foregroundStyle(Self.amber)
                .lineLimit(1).minimumScaleFactor(0.5)
            Text(hours > 0 ? "hours of music" : "minutes of music").font(.system(size: 15, weight: .bold))
            Text(detail).font(.system(size: 13)).foregroundStyle(.white.opacity(0.65)).padding(.top, 2)
            Text("By month").font(.system(size: 12)).foregroundStyle(.white.opacity(0.65)).padding(.top, 22).padding(.bottom, 8)
            HStack(alignment: .bottom, spacing: 5) {
                ForEach(review.months, id: \.month) { m in
                    RoundedRectangle(cornerRadius: 2)
                        .fill(m.seconds >= most ? Self.amber : Self.amber.opacity(0.42))
                        .frame(height: max(3, 110 * m.seconds / most))
                        .frame(maxWidth: .infinity)
                }
            }
            .frame(height: 110, alignment: .bottom)
            .accessibilityElement()
            .accessibilityLabel("Listening by month")
        }
    }

    private var artist: some View {
        card {
            VStack(spacing: 10) {
                Text("Top artist").font(.system(size: 12)).foregroundStyle(.white.opacity(0.65))
                if let top = review.topArtist {
                    photo(top.name).padding(.vertical, 8)
                    Text(top.name).font(.system(size: 22, weight: .bold)).multilineTextAlignment(.center).lineLimit(2)
                    Text(artistLine(top)).font(.system(size: 13)).foregroundStyle(.white.opacity(0.65)).multilineTextAlignment(.center)
                } else {
                    Text("Nobody yet").foregroundStyle(.white.opacity(0.65))
                }
            }
            .frame(maxWidth: .infinity)
        }
    }

    private func photo(_ name: String) -> some View {
        ZStack {
            if let image = images[Self.artistKey] {
                Image(nsImage: image).resizable().scaledToFill()
            } else {
                let p = SeedPalette(name)
                LinearGradient(colors: [p.colors[0], p.colors[1].opacity(0.7)], startPoint: .topLeading, endPoint: .bottomTrailing)
            }
        }
        .frame(width: 130, height: 130)
        .clipShape(Circle())
    }

    /// "212 hours · in your top 5 every month".
    private func artistLine(_ top: YearTopArtist) -> String {
        let hours = Int(top.seconds / 3600)
        let time = hours > 0 ? count(hours, "hour") : count(Int((top.seconds / 60).rounded()), "minute")
        let months = Int(top.monthsInTopFive)
        let place = months >= 12 ? "in your top 5 every month" : months > 1 ? "in your top 5 for \(months) months" : count(Int(top.plays), "play")
        return "\(time) · \(place)"
    }

    private var songs: some View {
        card {
            Text("Top songs").font(.system(size: 12)).foregroundStyle(.white.opacity(0.65)).padding(.bottom, 8)
            ForEach(Array(review.topSongs.prefix(Self.shown).enumerated()), id: \.offset) { i, top in
                HStack(spacing: 12) {
                    Text("\(i + 1)").font(.system(size: 13)).foregroundStyle(Self.amber).frame(width: 14)
                    cover(top.song)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(top.song.title).font(.system(size: 13.5, weight: .medium)).lineLimit(1)
                        Text(top.song.artist).font(.system(size: 12)).foregroundStyle(.white.opacity(0.65)).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    Text(count(Int(top.plays), "play")).font(.system(size: 12.5)).foregroundStyle(.white.opacity(0.55))
                }
                .padding(.vertical, 9)
                if i < min(review.topSongs.count, Self.shown) - 1 { Divider().overlay(.white.opacity(0.08)) }
            }
            if let actions {
                Spacer(minLength: 16)
                FlowLayout(spacing: 8) {
                    pill("Save as a picture", prominent: true, action: actions.picture)
                    pill("Make a playlist", action: actions.playlist)
                    pill("Play top 50", action: actions.play)
                }
            }
        }
    }

    private func cover(_ song: Song) -> some View {
        ZStack {
            if let path = song.artwork, let image = images[path] {
                Image(nsImage: image).resizable().scaledToFill()
            } else {
                let p = SeedPalette(song.album + song.artist)
                LinearGradient(colors: [p.colors[0], p.colors[1]], startPoint: .topLeading, endPoint: .bottomTrailing)
            }
        }
        .frame(width: 36, height: 36)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private func pill(_ title: String, prominent: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: 13.5, weight: .semibold))
                .lineLimit(1)
                .fixedSize()
                .foregroundStyle(prominent ? Color(red: 0.16, green: 0.1, blue: 0.03) : .white)
                .padding(.horizontal, 16).padding(.vertical, 10)
                .background(prominent ? Self.amber : .white.opacity(0.1), in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

/// Home's "Your year in music" card, in December.
struct YearHomeCard: View {
    private var store: Store { .shared }
    let year: Int32

    var body: some View {
        Button { store.navigate(.wrapped(Int(year))) } label: {
            HStack(spacing: 16) {
                Image(systemName: "sparkles").font(.system(size: 26)).foregroundStyle(Color(red: 0.886, green: 0.706, blue: 0.424))
                VStack(alignment: .leading, spacing: 3) {
                    Text("YOUR \(String(year)) IN MUSIC").font(.system(size: 11, design: .monospaced)).kerning(1.6)
                        .foregroundStyle(Color(red: 0.886, green: 0.706, blue: 0.424))
                    Text("Your year in music").font(.needle(.title2).weight(.semibold)).foregroundStyle(.white)
                    Text("Your hours, your top artist, and your top songs.").foregroundStyle(.white.opacity(0.7))
                }
                Spacer()
                Image(systemName: "chevron.right").foregroundStyle(.white.opacity(0.6))
            }
            .padding(20)
            .background {
                LinearGradient(colors: [Color(red: 0.16, green: 0.11, blue: 0.2), Color(red: 0.33, green: 0.2, blue: 0.11)], startPoint: .leading, endPoint: .trailing)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            }
        }
        .buttonStyle(.plain)
    }
}
