import Charts
import SwiftUI

func hoursMinutes(_ seconds: Double) -> String {
    let minutes = Int(seconds / 60)
    return minutes >= 60 ? "\(minutes / 60) h \(minutes % 60) min" : "\(minutes) min"
}

/// What was played, newest first, loaded a page at a time.
struct HistoryView: View {
    @Environment(Store.self) private var store
    @State private var items: [HistoryItem] = []
    @State private var done = false

    var body: some View {
        List {
            Section {
                NavigationLink(value: Route.stats) { Label("Listening numbers", systemImage: "chart.bar") }
                NavigationLink(value: Route.wrapped) { Label("Wrapped", systemImage: "gift") }
            }
            Section {
                ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                    Button {
                        if let song = item.song { store.play([song]) }
                    } label: {
                        HStack(spacing: 12) {
                            Artwork(path: item.song?.artwork, seed: item.album + item.artist, size: 120)
                                .frame(width: 44, height: 44).clipShape(.rect(cornerRadius: 7))
                            VStack(alignment: .leading, spacing: 2) {
                                Text(item.title).lineLimit(1).foregroundStyle(item.song == nil ? .secondary : .primary)
                                Text(item.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 2) {
                                Text(Date(timeIntervalSince1970: TimeInterval(item.startedAt)), format: .relative(presentation: .named))
                                    .font(.caption).foregroundStyle(.secondary)
                                if !item.counted { Text("skipped").font(.caption2).foregroundStyle(.tertiary) }
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .onAppear { if i == items.count - 1 { Task { await more() } } }
                }
            }
        }
        .overlay { if done && items.isEmpty { ContentUnavailableView("Nothing played yet", systemImage: "clock") } }
        .navigationTitle("History")
        .themed()
        .task(id: store.libraryVersion) { items = []; done = false; await more() }
    }

    private func more() async {
        guard !done else { return }
        let offset = UInt32(items.count)
        let page = await store.load { try $0.history(offset: offset, limit: 100) } ?? []
        if page.count < 100 { done = true }
        items += page
    }
}

/// Listening numbers: time, top artists, albums, and songs, and when you listen.
struct StatsView: View {
    @Environment(Store.self) private var store
    @State private var days: UInt32? = 30
    @State private var stats: Stats?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Picker("Over", selection: $days) {
                    Text("Week").tag(UInt32?.some(7))
                    Text("Month").tag(UInt32?.some(30))
                    Text("Year").tag(UInt32?.some(365))
                    Text("All time").tag(UInt32?.none)
                }
                .pickerStyle(.segmented)
                if let s = stats {
                    LazyVGrid(columns: [GridItem(), GridItem()], spacing: 12) {
                        figure(hoursMinutes(s.seconds), "listened")
                        figure(s.plays.formatted(), "plays")
                        figure(s.songs.formatted(), "songs")
                        figure(s.artists.formatted(), "artists")
                    }
                    if s.days.count > 1 {
                        Text("Each day").font(.headline)
                        Chart(s.days, id: \.date) { day in
                            BarMark(x: .value("Day", day.date), y: .value("Minutes", day.seconds / 60))
                                .foregroundStyle(.tint)
                        }
                        .chartXAxis(.hidden)
                        .frame(height: 140)
                    }
                    if s.hours.contains(where: { $0 > 0 }) {
                        Text("By hour").font(.headline)
                        Chart(Array(s.hours.enumerated()), id: \.offset) { hour, seconds in
                            BarMark(x: .value("Hour", hour), y: .value("Minutes", seconds / 60))
                                .foregroundStyle(.tint.opacity(0.8))
                        }
                        .chartXScale(domain: 0...23)
                        .frame(height: 120)
                    }
                    ranked("Top artists", s.topArtists) { r in .artist(r.title) }
                    ranked("Top albums", s.topAlbums) { _ in nil }
                    ranked("Top songs", s.topSongs) { _ in nil }
                }
            }
            .padding()
        }
        .overlay { if stats == nil { ProgressView() } }
        .navigationTitle("Listening")
        .themed()
        .task(id: days) {
            let d = days
            stats = await store.load { try $0.stats(days: d) }
        }
    }

    private func figure(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(value).font(.title2.bold())
            Text(label).font(.subheadline).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 18))
    }

    @ViewBuilder
    private func ranked(_ title: String, _ list: [Ranked], route: @escaping (Ranked) -> Route?) -> some View {
        if !list.isEmpty {
            Text(title).font(.headline).padding(.top, 6)
            VStack(spacing: 10) {
                ForEach(Array(list.prefix(10).enumerated()), id: \.offset) { i, r in
                    let row = HStack(spacing: 12) {
                        Text("\(i + 1)").font(.subheadline.monospacedDigit()).foregroundStyle(.secondary).frame(width: 22)
                        Artwork(path: r.artwork, seed: r.album + r.artist, size: 120)
                            .frame(width: 40, height: 40).clipShape(.rect(cornerRadius: 6))
                        VStack(alignment: .leading, spacing: 1) {
                            Text(r.title).lineLimit(1)
                            if r.artist != r.title && !r.artist.isEmpty { Text(r.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                        }
                        Spacer()
                        Text("\(r.plays)×").font(.caption).foregroundStyle(.secondary)
                    }
                    if let route = route(r) {
                        NavigationLink(value: route) { row }.buttonStyle(.plain)
                    } else if let id = r.songId {
                        Button { store.run { try $0.play(ids: [id], start: 0) }; store.played() } label: { row }.buttonStyle(.plain)
                    } else {
                        row
                    }
                }
            }
        }
    }
}

/// A year of listening, told a card at a time.
struct WrappedView: View {
    @Environment(Store.self) private var store
    @State private var years: [Int32] = []
    @State private var year: Int32?
    @State private var wrapped: Wrapped?

    var body: some View {
        ScrollView {
            VStack(spacing: 14) {
                if years.count > 1 {
                    Picker("Year", selection: $year) {
                        ForEach(years, id: \.self) { Text(String($0)).tag(Int32?.some($0)) }
                    }
                    .pickerStyle(.segmented)
                }
                if let w = wrapped {
                    card(String(w.year), "Your year in music", "\(hoursMinutes(w.stats.seconds)) of \(count(Int(w.stats.songs), "song")) by \(count(Int(w.stats.artists), "artist")).")
                    if let artist = w.stats.topArtists.first {
                        card("Top artist", artist.title, "\(count(Int(artist.plays), "play")), \(hoursMinutes(artist.seconds)).")
                    }
                    if let song = w.topSongs.first {
                        card("Song of the year", song.title, song.artist)
                    }
                    if !w.genres.isEmpty { card("Your sounds", w.genres.prefix(3).joined(separator: ", "), "") }
                    if !w.newArtists.isEmpty { card("New to you", w.newArtists.prefix(3).joined(separator: ", "), "Artists you found this year.") }
                    if let day = w.busiestDay { card("Busiest day", day.date, hoursMinutes(day.seconds) + " of music.") }
                    if w.streak > 1 { card("Longest streak", "\(w.streak) days", "In a row with music.") }
                    if let hour = w.peakHour { card("Your hour", "\(hour):00", "When you listen most.") }
                    if !w.topSongs.isEmpty {
                        Button { store.play(w.topSongs) } label: {
                            Label("Play your top songs", systemImage: "play.fill").frame(maxWidth: .infinity).padding(.vertical, 6)
                        }
                        .buttonStyle(.glassProminent)
                    }
                } else if years.isEmpty {
                    ContentUnavailableView("Nothing played yet", systemImage: "gift").padding(.top, 60)
                }
            }
            .padding()
        }
        .navigationTitle("Wrapped")
        .themed()
        .task {
            years = await store.load { try $0.wrappedYears() } ?? []
            year = years.first
        }
        .task(id: year) {
            guard let y = year else { return }
            wrapped = await store.load { try $0.wrapped(year: y) }
        }
    }

    private func card(_ kicker: String, _ title: String, _ detail: String) -> some View {
        let p = Palette(kicker + title)
        return VStack(alignment: .leading, spacing: 6) {
            Text(kicker.uppercased()).font(.caption.bold()).kerning(0.8).foregroundStyle(.white.opacity(0.8))
            Text(title).font(.largeTitle.bold()).foregroundStyle(.white).lineLimit(3).minimumScaleFactor(0.6)
            if !detail.isEmpty { Text(detail).font(.headline).foregroundStyle(.white.opacity(0.85)) }
        }
        .frame(maxWidth: .infinity, minHeight: 150, alignment: .bottomLeading)
        .padding(20)
        .background(LinearGradient(colors: [p.colors[0], p.colors[1].opacity(0.7)], startPoint: .topLeading, endPoint: .bottomTrailing), in: .rect(cornerRadius: 26))
    }
}

/// The music folders, folder by folder.
struct FoldersView: View {
    @Environment(Store.self) private var store
    let path: String?
    @State private var folders: [Folder]?
    @State private var songs: [Song] = []

    var body: some View {
        List {
            ForEach(folders ?? [], id: \.path) { folder in
                NavigationLink(value: Route.folder(folder.path)) {
                    HStack(spacing: 12) {
                        Artwork(path: folder.artwork, seed: folder.name, size: 120)
                            .frame(width: 44, height: 44).clipShape(.rect(cornerRadius: 7))
                            .overlay { if folder.artwork == nil { Image(systemName: "folder.fill").foregroundStyle(.white) } }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(folder.name)
                            Text(count(Int(folder.songs), "song")).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
            if !songs.isEmpty {
                Section {
                    PlayButtons(songs: songs).listRowBackground(Color.clear)
                    ForEach(Array(songs.enumerated()), id: \.element.id) { i, song in
                        SongRow(song: song) { store.play(songs, from: i) }
                    }
                }
            }
        }
        .listStyle(.plain)
        .overlay { if folders == nil { ProgressView() } }
        .navigationTitle(path.map { ($0 as NSString).lastPathComponent } ?? "Folders")
        .themed()
        .task(id: store.libraryVersion) {
            if let path {
                folders = await store.load { try $0.subfolders(path: path) } ?? []
                songs = await store.load { try $0.folderSongs(path: path) } ?? []
            } else {
                let roots = await store.load { try $0.folders() } ?? []
                folders = roots.map { Folder(path: $0, name: $0 == store.musicFolder.path ? "Needle's folder" : ($0 as NSString).lastPathComponent, songs: 0, artwork: nil) }
                if roots.count == 1, let only = roots.first {
                    folders = await store.load { try $0.subfolders(path: only) } ?? []
                    songs = await store.load { try $0.folderSongs(path: only) } ?? []
                }
            }
        }
    }
}
