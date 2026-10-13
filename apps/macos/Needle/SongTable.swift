import SwiftUI

/// One row of the song table: its place in the list, and the song.
struct SongRow: Identifiable, Hashable {
    let index: Int
    let track: Track
    var id: Int { index }

    var title: String { track.title }
    var artist: String { track.artist }
    var album: String { track.album }
    var albumArtist: String { track.albumArtist }
    var genre: String { track.genre }
    var year: Int { track.year }
    var format: String { track.format }
    var bitrate: Int { track.bitrate }
    var sampleRate: Int { track.sampleRate }
    var bpm: Double { track.bpm ?? 0 }
    var plays: Int { track.playCount }
    var lastPlayed: Int { track.lastPlayed ?? 0 }
    var added: Int { track.addedAt }
    var rating: Int { track.rating }
    var duration: Double { track.duration }
    var number: Int { track.trackNumber }
}

/// The song table: choose its columns (right-click the header), drag them to reorder or
/// resize, click one to sort. Double-click or Return plays; songs drag to playlists, the queue,
/// and Favorites; a playlist's own songs drag into a new order.
struct SongTable: View {
    @Environment(Store.self) private var store
    let model: SongsModel
    let route: Route
    @State private var selection: Set<Int> = []
    @State private var sortOrder: [KeyPathComparator<SongRow>] = []
    @AppStorage("song-columns") private var savedColumns = Data()
    @State private var columns = TableColumnCustomization<SongRow>()

    private var albumView: Bool { if case .album = route { true } else { false } }
    /// A playlist of picked songs in its own order: songs can be moved about and taken out.
    private var arrangeable: Bool { model.playlistID != nil && store.sort == nil && store.search.isEmpty }
    private var compact: Bool { store.settings.layout.rowHeight < 50 }

    private var rows: [SongRow] {
        model.tracks.enumerated().map { SongRow(index: model.offset + $0.offset, track: $0.element) }
    }

    private static let fields: [PartialKeyPath<SongRow>: String] = [
        \SongRow.title: "title", \SongRow.artist: "artist", \SongRow.album: "album", \SongRow.albumArtist: "album_artist",
        \SongRow.genre: "genre", \SongRow.year: "year", \SongRow.format: "format", \SongRow.bitrate: "bitrate",
        \SongRow.sampleRate: "sample_rate", \SongRow.bpm: "bpm", \SongRow.plays: "play_count",
        \SongRow.lastPlayed: "last_played", \SongRow.added: "added_at", \SongRow.rating: "rating",
        \SongRow.duration: "duration", \SongRow.number: "track_number",
    ]

    var body: some View {
        let colors = store.colors
        let rows = rows
        let playing = store.playback?.current
        Table(of: SongRow.self, selection: $selection, sortOrder: $sortOrder, columnCustomization: $columns) {
            Group {
            TableColumn("#", value: \.number) { row in
                ZStack(alignment: .trailing) {
                    if row.track.id == playing {
                        Equalizer(color: colors.accent, playing: store.playback?.playing == true).frame(width: 14, height: 12)
                    } else {
                        Text(albumView && store.sort == nil && row.number > 0 ? String(row.number) : (row.index + 1).formatted())
                            .foregroundStyle(colors.ink3)
                            .monospacedDigit()
                    }
                }
                .frame(maxWidth: .infinity, alignment: .trailing)
            }
            .width(min: 28, ideal: 36, max: 60)
            .customizationID("number")

            TableColumn("Title", value: \.title) { row in
                TitleCell(track: row.track, playing: row.track.id == playing, cover: !albumView, compact: compact,
                          showArtist: !albumView || (row.artist != row.albumArtist && !row.albumArtist.isEmpty))
            }
            .width(min: 180, ideal: 320)
            .customizationID("title")
            .disabledCustomizationBehavior(.visibility)

            TableColumn("Artist", value: \.artist) { row in Text(row.track.displayArtist).foregroundStyle(colors.ink2) }
                .width(min: 80, ideal: 160)
                .customizationID("artist")
                .defaultVisibility(.hidden)
            TableColumn("Album", value: \.album) { row in
                Text(row.track.displayAlbum).foregroundStyle(colors.ink2)
            }
            .width(min: 80, ideal: 220)
            .customizationID("album")
            .defaultVisibility(albumView ? .hidden : .automatic)
            TableColumn("Album artist", value: \.albumArtist) { row in Text(row.albumArtist).foregroundStyle(colors.ink2) }
                .width(min: 80, ideal: 160)
                .customizationID("album_artist")
                .defaultVisibility(.hidden)
            TableColumn("Genre", value: \.genre) { row in Text(row.genre).foregroundStyle(colors.ink2) }
                .width(min: 60, ideal: 120)
                .customizationID("genre")
                .defaultVisibility(.hidden)
            TableColumn("Year", value: \.year) { row in Text(row.year > 0 ? String(row.year) : "").monospacedDigit().foregroundStyle(colors.ink2) }
                .width(min: 40, ideal: 56)
                .customizationID("year")
                .defaultVisibility(.hidden)
            TableColumn("Quality", value: \.format) { row in Text(row.track.quality).font(.callout).foregroundStyle(colors.ink3) }
                .width(min: 60, ideal: 92)
                .customizationID("quality")
            }
            Group {
            TableColumn("Bitrate", value: \.bitrate) { row in Text(row.bitrate > 0 ? "\(row.bitrate) kbps" : "").monospacedDigit().foregroundStyle(colors.ink3) }
                .width(min: 50, ideal: 76)
                .customizationID("bitrate")
                .defaultVisibility(.hidden)
            TableColumn("Sample rate", value: \.sampleRate) { row in Text(khz(row.sampleRate)).monospacedDigit().foregroundStyle(colors.ink3) }
                .width(min: 60, ideal: 88)
                .customizationID("sample_rate")
                .defaultVisibility(.hidden)
            TableColumn("BPM", value: \.bpm) { row in Text(row.bpm > 0 ? String(format: "%.0f", row.bpm) : "").monospacedDigit().foregroundStyle(colors.ink3) }
                .width(min: 36, ideal: 52)
                .customizationID("bpm")
                .defaultVisibility(.hidden)
            TableColumn("Plays", value: \.plays) { row in Text(row.plays > 0 ? String(row.plays) : "").monospacedDigit().foregroundStyle(colors.ink3) }
                .width(min: 36, ideal: 52)
                .customizationID("plays")
                .defaultVisibility(.hidden)
            TableColumn("Last played", value: \.lastPlayed) { row in Text(day(row.track.lastPlayed)).foregroundStyle(colors.ink3) }
                .width(min: 70, ideal: 104)
                .customizationID("last_played")
                .defaultVisibility(.hidden)
            TableColumn("Added", value: \.added) { row in Text(day(row.added)).foregroundStyle(colors.ink3) }
                .width(min: 70, ideal: 96)
                .customizationID("added")
                .defaultVisibility(.hidden)
            TableColumn("Rating", value: \.rating) { row in
                Stars(rating: row.rating, size: 10) { store.rate([row.track.id], $0) }
            }
            .width(min: 70, ideal: 84)
            .customizationID("rating")
            .defaultVisibility(.hidden)
            TableColumn("") { row in
                Heart(on: row.track.favorite) { store.rate([row.track.id], row.track.favorite ? 0 : 5) }
            }
            .width(24)
            .customizationID("heart")
            TableColumn("Time", value: \.duration) { row in
                Text(clock(row.duration)).monospacedDigit().foregroundStyle(colors.ink3)
                    .frame(maxWidth: .infinity, alignment: .trailing)
            }
            .width(min: 44, ideal: 52, max: 80)
            .customizationID("time")
            }
        } rows: {
            ForEach(rows) { row in
                TableRow(row)
                    .draggable(row.track.id)
            }
            .dropDestination(for: String.self) { index, ids in
                guard arrangeable, let playlist = model.playlistID else { return }
                let before = index < rows.count ? rows[index].track.id : nil
                store.run { try $0.placeInPlaylist(id: playlist, ids: ids, before: before) }
                Task { try? await Task.sleep(for: .milliseconds(150)); store.libraryChanged() }
            }
        }
        .tableStyle(.inset(alternatesRowBackgrounds: false))
        .scrollContentBackground(.hidden)
        .environment(\.defaultMinListRowHeight, compact ? 26 : 44)
        .contextMenu(forSelectionType: Int.self) { ids in
            SongMenu(tracks: tracks(ids), route: route, arrangeable: arrangeable, playlistID: model.playlistID)
        } primaryAction: { ids in
            play(ids)
        }
        .onKeyPress(.return, phases: .down) { press in
            let ids = selection
            guard !ids.isEmpty else { return .ignored }
            if press.modifiers.contains(.shift) { store.playNext(tracks(ids).map(\.id)); return .handled }
            if press.modifiers.contains(.command) { store.enqueue(tracks(ids).map(\.id)); return .handled }
            play(ids)
            return .handled
        }
        .onDeleteCommand {
            guard arrangeable, let playlist = model.playlistID else { return }
            removeFromPlaylist(playlist, tracks(selection).map(\.id))
        }
        .onChange(of: selection) { _, ids in
            let chosen = tracks(ids)
            store.selectedTracks = chosen
            if chosen.count == 1 { store.focused = chosen[0] }
        }
        .onChange(of: sortOrder) { _, order in
            guard let first = order.first, let field = Self.fields[first.keyPath] else { store.sort = nil; return }
            store.sort = (field, first.order == .forward)
        }
        .onChange(of: columns) { _, value in savedColumns = (try? JSONEncoder().encode(value)) ?? Data() }
        .onAppear {
            if let saved = try? JSONDecoder().decode(TableColumnCustomization<SongRow>.self, from: savedColumns) { columns = saved }
            if let (field, ascending) = store.sort, let path = Self.fields.first(where: { $0.value == field })?.key {
                sortOrder = Self.comparator(path, ascending).map { [$0] } ?? []
            }
        }
    }

    private static func comparator(_ path: PartialKeyPath<SongRow>, _ ascending: Bool) -> KeyPathComparator<SongRow>? {
        let order: SortOrder = ascending ? .forward : .reverse
        switch path {
        case let p as KeyPath<SongRow, String>: return KeyPathComparator(p, order: order)
        case let p as KeyPath<SongRow, Int>: return KeyPathComparator(p, order: order)
        case let p as KeyPath<SongRow, Double>: return KeyPathComparator(p, order: order)
        default: return nil
        }
    }

    private func tracks(_ ids: Set<Int>) -> [Track] {
        model.tracks.enumerated().filter { ids.contains(model.offset + $0.offset) }.map(\.element)
    }

    /// Plays from the song double-clicked (all the page holds, so Repeat comes round), or the
    /// songs chosen when there are several.
    private func play(_ ids: Set<Int>) {
        let chosen = tracks(ids)
        if chosen.count > 1 {
            store.play(chosen)
        } else if let index = ids.first {
            store.playPage(rule: model.rule, playlist: model.playlistID, start: index)
        }
    }

    private func removeFromPlaylist(_ playlist: String, _ ids: [String]) {
        Task {
            guard let n = await store.ask({ try $0.removeSongsFromPlaylist(id: playlist, ids: ids) }) else { return }
            selection = []
            store.notify("Took \(count(Int(n), "song")) out of “\(model.playlist?.name ?? "the playlist")”.")
            store.libraryChanged()
        }
    }

    private func khz(_ rate: Int) -> String {
        guard rate > 0 else { return "" }
        let k = Double(rate) / 1000
        return k.rounded() == k ? "\(Int(k)) kHz" : String(format: "%.1f kHz", k)
    }
}

/// The title cell: the cover, the title (marked when the file is missing), and the artist.
struct TitleCell: View {
    @Environment(Store.self) private var store
    let track: Track
    let playing: Bool
    let cover: Bool
    let compact: Bool
    let showArtist: Bool

    var body: some View {
        let colors = store.colors
        HStack(spacing: 10) {
            if cover { Cover(path: track.artwork, seed: track.album + track.displayAlbumArtist, side: compact ? 22 : 36, corner: 4) }
            let title = HStack(spacing: 6) {
                Text(track.title)
                    .fontWeight(.medium)
                    .foregroundStyle(playing ? colors.accent : track.missing ? colors.ink3 : colors.ink)
                    .lineLimit(1)
                if track.missing {
                    Text("Missing file").font(.caption2).padding(.horizontal, 5).padding(.vertical, 1)
                        .background(colors.danger.opacity(0.14), in: RoundedRectangle(cornerRadius: 3))
                        .foregroundStyle(colors.danger)
                }
            }
            if compact {
                HStack(spacing: 8) {
                    title
                    if showArtist { Text(track.displayArtist).foregroundStyle(colors.ink2).lineLimit(1) }
                }
            } else {
                VStack(alignment: .leading, spacing: 2) {
                    title
                    if showArtist { Text(track.displayArtist).font(.callout).foregroundStyle(colors.ink2).lineLimit(1) }
                }
            }
        }
        .padding(.vertical, compact ? 1 : 3)
    }
}

/// The heart: a favorite or not.
struct Heart: View {
    @Environment(Store.self) private var store
    let on: Bool
    var size: CGFloat = 12
    let action: () -> Void
    @State private var pop = false

    var body: some View {
        Button {
            if !on && store.motion {
                withAnimation(.spring(response: 0.25, dampingFraction: 0.45)) { pop = true }
                Task { try? await Task.sleep(for: .milliseconds(260)); pop = false }
            }
            action()
        } label: {
            Image(systemName: on ? "heart.fill" : "heart")
                .font(.system(size: size))
                .foregroundStyle(on ? store.colors.accent : store.colors.ink3)
                .scaleEffect(pop ? 1.35 : 1)
        }
        .buttonStyle(.borderless)
        .help(on ? "Remove from favorites" : "Add to favorites")
    }
}

/// Five stars to rate with; clicking the stars a song has takes them away.
struct Stars: View {
    @Environment(Store.self) private var store
    let rating: Int
    var size: CGFloat = 14
    let set: (Int) -> Void

    var body: some View {
        HStack(spacing: 1) {
            ForEach(1...5, id: \.self) { star in
                Button { set(rating == star ? 0 : star) } label: {
                    Image(systemName: rating >= star ? "star.fill" : "star")
                        .font(.system(size: size))
                        .foregroundStyle(rating >= star ? store.colors.accent : store.colors.ink3)
                }
                .buttonStyle(.borderless)
                .help("Rate \(star) of 5")
            }
        }
    }
}

/// Bouncing bars for the song playing.
struct Equalizer: View {
    let color: Color
    let playing: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 12, paused: !playing)) { context in
            let t = context.date.timeIntervalSinceReferenceDate
            HStack(alignment: .bottom, spacing: 2) {
                ForEach(0..<3, id: \.self) { i in
                    let level = playing ? 0.35 + 0.65 * abs(sin(t * (3.1 + Double(i) * 1.7) + Double(i))) : 0.3
                    Capsule().fill(color).frame(width: 3).frame(height: 12 * level)
                }
            }
            .frame(height: 12, alignment: .bottom)
        }
    }
}
