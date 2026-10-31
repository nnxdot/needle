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

    @State private var loading = false
    /// Goes up each time the list begins again.
    @State private var generation = 0

    var body: some View {
        List {
            Section {
                NavigationLink(value: Route.stats) { Label("Listening numbers", systemImage: "chart.bar") }
                NavigationLink(value: Route.wrapped) { Label("Your year in music", systemImage: "sparkles") }
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
                                Text(item.artist).font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 2) {
                                Text(Date(timeIntervalSince1970: TimeInterval(item.startedAt)), format: .relative(presentation: .named))
                                    .font(.needle(.caption)).foregroundStyle(.secondary)
                                if !item.counted { Text("skipped").font(.needle(.caption2)).foregroundStyle(.tertiary) }
                            }
                        }
                    }
                    .buttonStyle(.plain)
                    .onAppear { if i == items.count - 1 { Task { await more() } } }
                }
            }
        }
        .overlay { if done && items.isEmpty { ContentUnavailableView("Nothing played yet", systemImage: "clock") } }
        .navigationTitle("Listening history")
        .themed()
        .task(id: store.libraryVersion) { generation += 1; loading = false; items = []; done = false; await more() }
    }

    private func more() async {
        guard !done, !loading else { return }
        loading = true
        let (offset, from) = (UInt32(items.count), generation)
        let page = await store.load { try $0.history(offset: offset, limit: 100) } ?? []
        guard from == generation, UInt32(items.count) == offset else { return }
        loading = false
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
                        Text("Each day").font(.needle(.headline))
                        Chart(s.days, id: \.date) { day in
                            BarMark(x: .value("Day", day.date), y: .value("Minutes", day.seconds / 60))
                                .foregroundStyle(.tint)
                        }
                        .chartXAxis(.hidden)
                        .frame(height: 140)
                    }
                    if s.hours.contains(where: { $0 > 0 }) {
                        Text("By hour").font(.needle(.headline))
                        Chart(Array(s.hours.enumerated()), id: \.offset) { hour, seconds in
                            BarMark(x: .value("Hour", hour), y: .value("Minutes", seconds / 60))
                                .foregroundStyle(.tint.opacity(0.8))
                        }
                        .chartXScale(domain: 0...23)
                        .frame(height: 120)
                    }
                    // Artists' and albums' rows carry their name in `artist` and `album` (only a
                    // song's row has a title).
                    ranked("Top artists", s.topArtists) { r in r.artist.isEmpty ? nil : .artist(r.artist) }
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
            Text(value).font(.needle(.title2).bold())
            Text(label).font(.needle(.subheadline)).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .glassEffect(.regular, in: .rect(cornerRadius: 18))
    }

    @ViewBuilder
    private func ranked(_ title: String, _ list: [Ranked], route: @escaping (Ranked) -> Route?) -> some View {
        if !list.isEmpty {
            Text(title).font(.needle(.headline)).padding(.top, 6)
            VStack(spacing: 10) {
                ForEach(Array(list.prefix(10).enumerated()), id: \.offset) { i, r in
                    let row = HStack(spacing: 12) {
                        Text("\(i + 1)").font(.needle(.subheadline).monospacedDigit()).foregroundStyle(.secondary).frame(width: 22)
                        Artwork(path: r.artwork, seed: r.album + r.artist, size: 120)
                            .frame(width: 40, height: 40).clipShape(.rect(cornerRadius: 6))
                        VStack(alignment: .leading, spacing: 1) {
                            let name = !r.title.isEmpty ? r.title : !r.album.isEmpty ? r.album : r.artist
                            Text(name).lineLimit(1)
                            if r.artist != name && !r.artist.isEmpty { Text(r.artist).font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1) }
                        }
                        Spacer()
                        Text("\(r.plays)×").font(.needle(.caption)).foregroundStyle(.secondary)
                    }
                    if let route = route(r) {
                        NavigationLink(value: route) { row }.buttonStyle(.plain)
                    } else if let id = r.songId {
                        Button { store.playIDs { _ in [id] } } label: { row }.buttonStyle(.plain)
                    } else {
                        row
                    }
                }
            }
        }
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
                            Text(count(Int(folder.songs), "song")).font(.needle(.caption)).foregroundStyle(.secondary)
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
                folders = roots.map { Folder(path: $0, name: Store.realPath($0) == Store.realPath(store.musicFolder.path) ? "Needle's folder" : ($0 as NSString).lastPathComponent, songs: 0, artwork: nil) }
                if roots.count == 1, let only = roots.first {
                    folders = await store.load { try $0.subfolders(path: only) } ?? []
                    songs = await store.load { try $0.folderSongs(path: only) } ?? []
                }
            }
        }
    }
}
