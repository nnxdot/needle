import Charts
import SwiftUI

/// Listening history: the numbers of a stretch of time, and every listen, newest first.
struct HistoryPage: View {
    @Environment(Store.self) private var store
    @State private var days: UInt32? = 30
    @State private var stats: Stats?
    @State private var items: [HistoryItem] = []
    @State private var finished = false

    var body: some View {
        let colors = store.colors
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Listening history").font(store.display(28))
                        Text("Every listen is recorded on this Mac, even offline.").foregroundStyle(colors.ink2)
                    }
                    Spacer()
                    Button { store.navigate(.wrapped(Calendar.current.component(.year, from: Date()))) } label: {
                        Label("Your year in music", systemImage: "sparkles")
                    }
                    .glassButton()
                }
                Picker("", selection: $days) {
                    Text("Week").tag(UInt32?.some(7))
                    Text("Month").tag(UInt32?.some(30))
                    Text("Year").tag(UInt32?.some(365))
                    Text("All time").tag(UInt32?.none)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 360)
                if let s = stats {
                    if s.listens == 0 {
                        VStack(spacing: 8) {
                            Text("Your listening story starts here").font(.title3.weight(.medium))
                            Text("Play something. Every listen is recorded on this Mac, even offline.").foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity)
                        .padding(40)
                    } else {
                        HStack(spacing: 12) {
                            figure(longTime(s.seconds), "listened")
                            figure(s.plays.formatted(), "plays")
                            figure(s.songs.formatted(), "songs")
                            figure(s.artists.formatted(), "artists")
                        }
                        HStack(alignment: .top, spacing: 20) {
                            if s.days.count > 1 {
                                VStack(alignment: .leading, spacing: 8) {
                                    Text(s.days.count > 60 ? "Listening per week" : "Listening per day").font(.headline)
                                    Chart(s.days, id: \.date) { day in
                                        BarMark(x: .value("Day", day.date), y: .value("Minutes", day.seconds / 60))
                                            .foregroundStyle(colors.accent.gradient)
                                    }
                                    .chartXAxis(.hidden)
                                    .frame(height: 150)
                                }
                                .frame(maxWidth: .infinity)
                            }
                            if s.hours.contains(where: { $0 > 0 }) {
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("Time of day").font(.headline)
                                    Chart(Array(s.hours.enumerated()), id: \.offset) { hour, seconds in
                                        BarMark(x: .value("Hour", hour), y: .value("Minutes", seconds / 60))
                                            .foregroundStyle(colors.accent.opacity(0.75))
                                    }
                                    .chartXScale(domain: 0...23)
                                    .frame(height: 150)
                                }
                                .frame(maxWidth: 360)
                            }
                        }
                        HStack(alignment: .top, spacing: 20) {
                            ranked("Top artists", s.topArtists) { r in store.navigate(.artist(r.title)) }
                            ranked("Top albums", s.topAlbums, play: nil)
                            ranked("Top tracks", s.topSongs, play: nil)
                        }
                    }
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(30)
                }
                if !items.isEmpty {
                    Text("Every listen, newest first").font(.title3.weight(.semibold))
                    LazyVStack(alignment: .leading, spacing: 2) {
                        ForEach(Array(runs.enumerated()), id: \.offset) { i, run in
                            listen(run.item, plays: run.count)
                                .onAppear { if i == runs.count - 1 { Task { await more() } } }
                        }
                    }
                }
            }
            .padding(28)
        }
        .task(id: days) {
            let d = days
            stats = await store.load { try $0.stats(days: d) }
        }
        .task(id: store.libraryVersion) {
            items = []
            finished = false
            await more()
        }
    }

    /// Runs of the same song played back to back, as one row.
    private var runs: [(item: HistoryItem, count: Int)] {
        var list: [(item: HistoryItem, count: Int)] = []
        for item in items {
            if let last = list.last, last.item.title == item.title, last.item.artist == item.artist {
                list[list.count - 1].count += 1
            } else {
                list.append((item, 1))
            }
        }
        return list
    }

    private func more() async {
        guard !finished else { return }
        let offset = UInt32(items.count)
        let page = await store.load { try $0.history(offset: offset, limit: 200) } ?? []
        if page.count < 200 { finished = true }
        items += page
    }

    private func listen(_ item: HistoryItem, plays: Int) -> some View {
        HStack(spacing: 12) {
            Cover(path: item.song?.artwork, seed: item.album + item.artist, side: 38, corner: 4)
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 6) {
                    Text(item.title).lineLimit(1).foregroundStyle(item.song == nil ? .secondary : .primary)
                    if plays > 1 { Text("×\(plays)").font(.caption).foregroundStyle(.secondary) }
                }
                Text("\(item.artist) · \(item.album)").font(.caption).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            Spacer()
            if !item.counted { Text("skipped").font(.caption2).foregroundStyle(.tertiary) }
            Text(when(item.startedAt)).font(.caption).foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture(count: 2) { if let song = item.song { store.play([song.asTrack], reason: "Played again from your history") } }
        .contextMenu { if let song = item.song { SongMenu(tracks: [song.asTrack]) } }
    }

    private func when(_ seconds: Int64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let time = date.formatted(date: .omitted, time: .shortened)
        if Calendar.current.isDateInToday(date) { return "Today · \(time)" }
        if Calendar.current.isDateInYesterday(date) { return "Yesterday · \(time)" }
        return date.formatted(.dateTime.weekday(.abbreviated).day().month(.abbreviated)) + " · " + time
    }

    private func figure(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value).font(.title2.bold())
            Text(label).font(.callout).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .pane(RoundedRectangle(cornerRadius: 16, style: .continuous), glass: store.glass, fill: store.colors.raised)
    }

    @ViewBuilder
    private func ranked(_ title: String, _ list: [Ranked], play: ((Ranked) -> Void)?) -> some View {
        if !list.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text(title).font(.headline)
                ForEach(Array(list.prefix(10).enumerated()), id: \.offset) { i, r in
                    HStack(spacing: 10) {
                        Text("\(i + 1)").font(.callout.monospacedDigit()).foregroundStyle(.secondary).frame(width: 20)
                        Cover(path: r.artwork, seed: r.album + r.artist, side: 34, corner: 4)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(r.title).lineLimit(1)
                            if r.artist != r.title && !r.artist.isEmpty { Text(r.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                        }
                        Spacer()
                        Text("\(r.plays)×").font(.caption).foregroundStyle(.secondary)
                    }
                    .contentShape(Rectangle())
                    .onTapGesture(count: 2) {
                        if let play { play(r) } else if let id = r.songId {
                            store.run { _ = try $0.playSongs(ids: [id], start: 0, reason: "One of your most played") }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }
}

/// A year of listening, told back.
struct WrappedPage: View {
    @Environment(Store.self) private var store
    let year: Int
    @State private var years: [Int32] = []
    @State private var wrapped: Wrapped?
    @State private var loading = true

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack {
                    Text("Your \(String(year)) in music").font(store.display(32))
                    Spacer()
                    if years.count > 1 {
                        Picker("Year", selection: Binding(get: { Int32(year) }, set: { store.navigate(.wrapped(Int($0))) })) {
                            ForEach(years, id: \.self) { Text(String($0)).tag($0) }
                        }
                        .fixedSize()
                    }
                }
                if loading {
                    HStack { ProgressView().controlSize(.small); Text("Gathering your year…").foregroundStyle(.secondary) }
                } else if let w = wrapped, w.stats.listens > 0 {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 16)], spacing: 16) {
                        card("Your \(w.year) in music", "You spent \(longTime(w.stats.seconds)) with music", "\(count(Int(w.stats.songs), "song")) by \(count(Int(w.stats.artists), "artist")).")
                        if let artist = w.stats.topArtists.first {
                            card("Your artist of the year", artist.title, "\(count(Int(artist.plays), "play")), \(longTime(artist.seconds)).")
                        }
                        if let album = w.stats.topAlbums.first {
                            card("Albums you lived in", album.title, w.stats.topAlbums.dropFirst().prefix(2).map(\.title).joined(separator: ", "))
                        }
                        if !w.newArtists.isEmpty {
                            card("New to you", w.newArtists.prefix(3).joined(separator: ", "), "Artists you first played in \(w.year).")
                        }
                        if let day = w.busiestDay {
                            card("Your biggest day", day.date, longTime(day.seconds) + " of music.")
                        }
                        if w.streak > 1 { card("Your longest streak", "\(w.streak) days", "Days with music, in a row.") }
                        if let hour = w.peakHour {
                            card("Your rhythm", rhythm(Int(hour)), "You listen most around \(hour):00.")
                        }
                        if !w.genres.isEmpty { card("What it sounded like", w.genres.prefix(3).joined(separator: ", "), "") }
                    }
                    if !w.topSongs.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Top songs of \(String(w.year))").font(.title3.weight(.semibold))
                                Spacer()
                                Button { store.play(w.topSongs.map(\.asTrack), reason: "One of your songs of the year") } label: {
                                    Label("Play your top \(w.topSongs.count)", systemImage: "play.fill")
                                }
                                .glassButton(prominent: true)
                                Button("Save as a playlist") { save(w) }.glassButton()
                            }
                            ForEach(Array(w.topSongs.enumerated()), id: \.offset) { i, song in
                                HStack(spacing: 10) {
                                    Text("\(i + 1)").monospacedDigit().foregroundStyle(.secondary).frame(width: 24)
                                    Cover(path: song.artwork, seed: song.album + song.artist, side: 36, corner: 4)
                                    VStack(alignment: .leading, spacing: 1) {
                                        Text(song.title).lineLimit(1)
                                        Text(song.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                    }
                                    Spacer()
                                }
                                .contentShape(Rectangle())
                                .onTapGesture(count: 2) { store.play(w.topSongs.map(\.asTrack), from: i, reason: "One of your songs of the year") }
                            }
                        }
                    }
                } else {
                    Text("No listening in \(String(year)) yet. Play some music and come back.").foregroundStyle(.secondary)
                }
            }
            .padding(28)
        }
        .task(id: year) {
            loading = true
            years = await store.load { try $0.wrappedYears() } ?? []
            let y = Int32(year)
            wrapped = await store.load { try $0.wrapped(year: y) }
            loading = false
        }
    }

    private func rhythm(_ hour: Int) -> String {
        switch hour {
        case 5..<11: "An early riser."
        case 11..<17: "An afternoon listener."
        case 17..<22: "An evening listener."
        default: "A night owl."
        }
    }

    private func card(_ kicker: String, _ title: String, _ detail: String) -> some View {
        let p = SeedPalette(kicker + title)
        return VStack(alignment: .leading, spacing: 6) {
            Text(kicker.uppercased()).font(.caption.bold()).kerning(0.8).foregroundStyle(.white.opacity(0.8))
            Text(title).font(store.display(28)).foregroundStyle(.white).lineLimit(3).minimumScaleFactor(0.6)
            if !detail.isEmpty { Text(detail).font(.headline).foregroundStyle(.white.opacity(0.85)) }
        }
        .frame(maxWidth: .infinity, minHeight: 160, alignment: .bottomLeading)
        .padding(20)
        .background(LinearGradient(colors: [p.colors[0], p.colors[1].opacity(0.75)], startPoint: .topLeading, endPoint: .bottomTrailing),
                    in: RoundedRectangle(cornerRadius: 22, style: .continuous))
    }

    private func save(_ w: Wrapped) {
        var playlist = PlaylistModel.new()
        playlist.name = "Top songs of \(w.year)"
        playlist.trackIds = w.topSongs.map(\.id)
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        guard let data = try? encoder.encode(playlist) else { return }
        let json = String(decoding: data, as: UTF8.self)
        Task {
            if await store.ask({ try $0.savePlaylistJson(json: json) }) != nil {
                store.notify("Saved “\(playlist.name)”.")
                store.libraryChanged()
            }
        }
    }
}
