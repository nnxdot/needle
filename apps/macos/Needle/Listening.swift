import Charts
import SwiftUI

/// Listening history: the numbers of a stretch of time, and every listen, newest first.
struct HistoryPage: View {
    private var store: Store { .shared }
    @State private var days: UInt32? = 30
    @State private var stats: Stats?
    @State private var items: [HistoryItem] = []
    @State private var finished = false
    @State private var loadingMore = false
    /// Goes up each time the list begins again, so a late page from before is left out.
    @State private var generation = 0

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
                    Button { store.navigate(.wrapped(store.reviewYear)) } label: {
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
                            Text("Your listening story starts here").font(.needle(.title3).weight(.medium))
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
                                    Text(s.days.count > 60 ? "Listening per week" : "Listening per day").font(.needle(.headline))
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
                                    Text("Time of day").font(.needle(.headline))
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
                            // Artists' and albums' rows carry their name in `artist` and `album`
                            // (only a song's row has a title).
                            ranked("Top artists", s.topArtists) { r in store.navigate(.artist(r.artist)) }
                            ranked("Top albums", s.topAlbums) { r in
                                // Its album's page, by its name and artist (as an album artist or a song's).
                                store.navigate(.album(album: r.album, artist: r.artist,
                                                      query: "album = \(Rule.quote(r.album)) and (album_artist = \(Rule.quote(r.artist)) or artist = \(Rule.quote(r.artist)))"))
                            }
                            ranked("Top tracks", s.topSongs, play: nil)
                        }
                    }
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(30)
                }
                if !items.isEmpty {
                    Text("Every listen, newest first").font(.needle(.title3).weight(.semibold))
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
            generation += 1
            loadingMore = false
            // As many as were shown, so the list stays where it was scrolled: in pages of at most
            // 500 (the core's most at a time).
            let want = max(200, items.count)
            let from = generation
            var all: [HistoryItem] = []
            var ended = false
            while all.count < want {
                let (offset, limit) = (UInt32(all.count), UInt32(min(500, want - all.count)))
                let page = await store.load { try $0.history(offset: offset, limit: limit) } ?? []
                all += page
                if page.count < Int(limit) { ended = true; break }
            }
            guard from == generation else { return }
            items = all
            finished = ended
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
        // One page at a time, and none from before the list began again.
        guard !finished, !loadingMore else { return }
        loadingMore = true
        let (offset, from) = (UInt32(items.count), generation)
        let page = await store.load { try $0.history(offset: offset, limit: 200) } ?? []
        guard from == generation else { return }
        loadingMore = false
        if page.count < 200 { finished = true }
        items += page
    }

    private func listen(_ item: HistoryItem, plays: Int) -> some View {
        HStack(spacing: 12) {
            Cover(path: item.song?.artwork, seed: item.album + item.artist, side: 38, corner: 4)
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 6) {
                    Text(item.title).lineLimit(1).foregroundStyle(item.song == nil ? .secondary : .primary)
                    if plays > 1 { Text("×\(plays)").font(.needle(.caption)).foregroundStyle(.secondary) }
                }
                Text("\(item.artist) · \(item.album)").font(.needle(.caption)).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            Spacer()
            if !item.counted { Text("skipped").font(.needle(.caption2)).foregroundStyle(.tertiary) }
            Text(when(item.startedAt)).font(.needle(.caption)).foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture(count: 2) { if let song = item.song { store.play([song.asTrack], reason: "Played again from your history") } }
        .contextMenu { if let song = item.song { SongMenu(tracks: [song.fullTrack]) } }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(item.song == nil ? [] : .isButton)
        .accessibilityAction { if let song = item.song { store.play([song.asTrack], reason: "Played again from your history") } }
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
            Text(value).font(.needle(.title2).bold())
            Text(label).font(.needle(.callout)).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .pane(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    /// A ranked row opened: its page, or its song played.
    private func open(_ r: Ranked, _ play: ((Ranked) -> Void)?) {
        if let play { play(r) } else if let id = r.songId {
            store.run { _ = try $0.playSongs(ids: [id], start: 0, reason: "One of your most played") }
        } else {
            store.notify("That song's file is gone.")
        }
    }

    /// A ranked row's name: a song's title, else its album, else its artist.
    private func name(_ r: Ranked) -> String {
        !r.title.isEmpty ? r.title : !r.album.isEmpty ? r.album : r.artist
    }

    @ViewBuilder
    private func ranked(_ title: String, _ list: [Ranked], play: ((Ranked) -> Void)?) -> some View {
        if !list.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text(title).font(.needle(.headline))
                ForEach(Array(list.prefix(10).enumerated()), id: \.offset) { i, r in
                    HStack(spacing: 10) {
                        Text("\(i + 1)").font(.needle(.callout).monospacedDigit()).foregroundStyle(.secondary).frame(width: 20)
                        Cover(path: r.artwork, seed: r.album + r.artist, side: 34, corner: 4)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(name(r)).lineLimit(1)
                            if r.artist != name(r) && !r.artist.isEmpty { Text(r.artist).font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1) }
                        }
                        Spacer()
                        Text("\(r.plays)×").font(.needle(.caption)).foregroundStyle(.secondary)
                    }
                    .contentShape(Rectangle())
                    .onTapGesture(count: 2) { open(r, play) }
                    .accessibilityElement(children: .combine)
                    .accessibilityAddTraits(.isButton)
                    .accessibilityAction { open(r, play) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }
}

