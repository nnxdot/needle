import SwiftUI

/// The side panel: the chosen song's details, the queue, or the lyrics.
struct Inspector: View {
    @Environment(Store.self) private var store

    var body: some View {
        @Bindable var store = store
        VStack(spacing: 0) {
            Picker("", selection: $store.panel) {
                ForEach(Store.Panel.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            Group {
                switch store.panel {
                case .details: Details()
                case .queue: QueuePanel()
                case .lyrics: LyricsView(kind: .side).padding(.horizontal, 16)
                }
            }
            .frame(maxHeight: .infinity, alignment: .top)
            .transition(.opacity)
        }
        .background { if !store.glass { store.colors.canvas.ignoresSafeArea() } }
    }
}

/// The chosen song (or the song playing): its cover, stars, facts, tags, stems, and lookups.
struct Details: View {
    @Environment(Store.self) private var store
    @State private var matches: [RecordingMatch] = []
    @State private var looking = false
    @State private var sound: String?

    var body: some View {
        let selected = store.selectedTracks
        if selected.count > 1 {
            many(selected)
        } else if let track = store.focused ?? store.current {
            one(track)
        } else {
            VStack(alignment: .leading, spacing: 6) {
                Text("Nothing selected").font(.headline)
                Text("Select a track to see its details, rate it, or fix its tags.").foregroundStyle(.secondary)
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func many(_ tracks: [Track]) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text("\(tracks.count) tracks selected").font(.title3.weight(.semibold))
                Text(clock(tracks.reduce(0) { $0 + $1.duration })).foregroundStyle(.secondary)
                HStack {
                    Button { store.play(tracks) } label: { Label("Play", systemImage: "play.fill") }.glassButton(prominent: true)
                    Button { store.enqueue(tracks.map(\.id)) } label: { Label("Add to queue", systemImage: "text.badge.plus") }.glassButton()
                }
                Button { store.tagging = tracks.map(\.id) } label: { Label("Edit tags on \(tracks.count) tracks", systemImage: "pencil") }
                    .glassButton()
                Text("Shift-click selects a range. ⌘-click adds or removes one track. Right-click for more.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func one(_ track: Track) -> some View {
        let colors = store.colors
        let source = track.sourcePlugin.map { id in store.plugins.first { $0.id == id }?.source?.name ?? id }
        return ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Cover(path: track.artwork, seed: track.album + track.displayAlbumArtist, side: 270, corner: 10)
                    .frame(maxWidth: .infinity)
                    .shadow(color: colors.glow.opacity(0.3), radius: 18, y: 8)
                VStack(alignment: .leading, spacing: 3) {
                    Text(track.title).font(.title3.weight(.semibold)).textSelection(.enabled)
                    Button(track.displayArtist) { store.navigate(.artist(track.artist)) }.buttonStyle(.link)
                    Button(track.displayAlbum) { store.navigate(track.albumRoute) }.buttonStyle(.link).foregroundStyle(colors.ink2)
                }
                HStack {
                    Stars(rating: track.rating, size: 15) { store.rate([track.id], $0) }
                    Spacer()
                    Button { store.enqueue([track.id]) } label: { Image(systemName: "text.badge.plus") }
                        .buttonStyle(.borderless).help("Add to queue").disabled(track.missing)
                    if source == nil {
                        Button { store.tagging = [track.id] } label: { Image(systemName: "pencil") }
                            .buttonStyle(.borderless).help("Edit tags · ⌘E")
                    }
                    Button { store.play([track], reason: "Chosen from track details") } label: { Label("Play", systemImage: "play.fill") }
                        .glassButton(prominent: true).controlSize(.small).disabled(track.missing)
                }
                if source == nil {
                    Divider()
                    Text("Stems").font(.caption).foregroundStyle(.secondary)
                    StemsPanel(track: track, big: false)
                }
                Divider()
                VStack(alignment: .leading, spacing: 0) {
                    fact("Quality", track.quality)
                    fact("Sample rate", String(format: "%.1f kHz · ", Double(track.sampleRate) / 1000) + channels(track.channels))
                    fact("Length", clock(track.duration))
                    fact("Year", track.year > 0 ? String(track.year) : "—")
                    fact("Genre", track.genre.isEmpty ? "—" : track.genre)
                    fact("Sound", sound ?? track.bpm.map { String(format: "%.0f BPM", $0) } ?? "Not measured yet")
                    fact("Plays", track.playCount == 0 ? "Never played" : "\(track.playCount)" + (track.lastPlayed.map { " · last \(day($0))" } ?? ""))
                    fact("Added", day(track.addedAt))
                    HStack {
                        fact("ReplayGain", track.replayGain.map { String(format: "%+.2f dB", $0) } ?? "Not measured")
                        Button(track.replayGain == nil ? "Measure" : "Remeasure") {
                            let id = track.id
                            store.notify("Measuring loudness in the background…")
                            store.job { try $0.measureLoudness(id: id) }
                        }
                        .buttonStyle(.borderless).font(.caption)
                    }
                    HStack {
                        fact("Album gain", track.albumReplayGain.map { String(format: "%+.2f dB", $0) } ?? "Not measured")
                        Button("Measure") {
                            let id = track.id
                            store.notify("Measuring the whole album in the background…")
                            store.job { try $0.measureAlbumLoudness(id: id) }
                        }
                        .buttonStyle(.borderless).font(.caption).help("Measure the whole album")
                    }
                    if let source {
                        let kept = store.core.keptState(id: track.id)
                        fact("From", kept.first == true ? "\(source) · kept on this computer" : kept.last == true ? "\(source) · played lately, so it is on this computer" : source)
                    } else {
                        HStack {
                            fact("File", track.filePath)
                            Button { store.copy(track.filePath) } label: { Image(systemName: "doc.on.doc") }
                                .buttonStyle(.borderless).help("Copy file path")
                            Button { store.reveal(track.filePath) } label: { Image(systemName: "magnifyingglass") }
                                .buttonStyle(.borderless).help("Show in Finder")
                        }
                    }
                }
                Divider()
                Text("Fix metadata online").font(.caption).foregroundStyle(.secondary)
                HStack {
                    Button { lookUp(track, sound: false) } label: { Label(looking ? "Looking up…" : "MusicBrainz", systemImage: "globe") }
                        .disabled(looking)
                        .help("Search MusicBrainz by artist and title. Sends only that text.")
                    if source == nil {
                        Button("Identify by sound") { lookUp(track, sound: true) }
                            .disabled(looking)
                            .help("Send an audio fingerprint to AcoustID. Needs an AcoustID key.")
                    }
                }
                .controlSize(.small)
                ForEach(matches) { m in
                    VStack(alignment: .leading, spacing: 3) {
                        Text(m.title).font(.callout.weight(.semibold))
                        Text("\(m.artist) · \(m.album)").font(.caption).foregroundStyle(.secondary)
                        HStack {
                            Button("Use these tags") { useTags(track, m) }
                            if let release = m.releaseId {
                                Button("Use its cover") {
                                    let id = track.id
                                    store.job { try $0.useReleaseCover(id: id, release: release); return "Cover art saved to the library cache." }
                                }
                            }
                        }
                        .controlSize(.mini)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(colors.raised, in: RoundedRectangle(cornerRadius: 8))
                }
            }
            .padding(16)
        }
        .task(id: track.id) {
            matches = []
            let id = track.id
            sound = await store.load { $0.soundSummary(id: id) } ?? nil
        }
    }

    private func fact(_ key: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(key).font(.caption).foregroundStyle(.secondary).frame(width: 82, alignment: .leading)
            Text(value).font(.callout).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
    }

    private func channels(_ n: Int) -> String {
        switch n {
        case 0: "—"
        case 1: "mono"
        case 2: "stereo"
        default: "\(n) channels"
        }
    }

    private func lookUp(_ track: Track, sound: Bool) {
        looking = true
        let id = track.id
        Task {
            let json = await store.ask { sound ? try $0.acoustidLookupJson(id: id) : try $0.musicbrainzLookup(id: id) }
            looking = false
            matches = JSON.decode([RecordingMatch].self, json) ?? []
            if json != nil && matches.isEmpty {
                store.notify("No matching recordings found. Try correcting the artist or title first.")
            }
        }
    }

    private func useTags(_ track: Track, _ m: RecordingMatch) {
        let edit: [String: Any] = ["title": m.title, "artist": m.artist, "album": m.album, "musicbrainz_id": m.id]
        let text = JSON.text(edit)
        let id = track.id
        Task {
            guard let report = await store.ask({ try $0.writeTagsJson(ids: [id], edit: text) }) else { return }
            if report.failed.isEmpty { store.notify("Tags saved. The original file is backed up in your library folder.") }
            else { store.fail(report.failed.joined(separator: "\n")) }
            matches = []
            store.libraryChanged()
        }
    }
}

/// Now playing, and the songs up next: double-click one to play it, drag to reorder.
struct QueuePanel: View {
    @Environment(Store.self) private var store

    var body: some View {
        let colors = store.colors
        VStack(alignment: .leading, spacing: 8) {
            if let p = store.playback, p.hasSong {
                HStack(spacing: 10) {
                    Cover(path: p.artwork, seed: p.album + p.artist, side: 46, corner: 5)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Now playing").font(.caption).foregroundStyle(.secondary)
                        Text(p.title).font(.callout.weight(.semibold)).foregroundStyle(colors.accent).lineLimit(1)
                        Text(p.artist).font(.caption).foregroundStyle(colors.ink2).lineLimit(1)
                        Text(p.reason).font(.caption2).foregroundStyle(colors.ink3).lineLimit(1)
                    }
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(colors.raised, in: RoundedRectangle(cornerRadius: 10))
                .padding(.horizontal, 14)
            }
            HStack(alignment: .firstTextBaseline) {
                Text("Next up").font(.headline)
                if let n = store.playback?.queue, n > 0 {
                    Text("\(n.formatted()) · \(clock(store.queueSeconds))").font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                if store.playback?.queue ?? 0 > 0 {
                    Button("Clear") { store.core.clearQueue() }.buttonStyle(.borderless).font(.caption)
                }
            }
            .padding(.horizontal, 16)
            if store.queue.isEmpty {
                Text(store.settings.autoplayQuery.trimmingCharacters(in: .whitespaces).isEmpty
                     ? "Nothing queued. Right-click any track and choose Add to Queue."
                     : "Nothing queued. Your autoplay rule picks what comes next.")
                    .font(.callout).foregroundStyle(.secondary).padding(.horizontal, 16)
                Spacer()
            } else {
                List {
                    ForEach(Array(store.queue.enumerated()), id: \.offset) { index, entry in
                        QueueRow(entry: entry, index: index)
                    }
                    .onMove { from, to in
                        guard let first = from.first else { return }
                        store.core.moveUpNext(from: UInt32(first), to: UInt32(to > first ? to - 1 : to))
                    }
                    if let n = store.playback?.queue, n > UInt32(store.queue.count) {
                        Text("And \((Int(n) - store.queue.count).formatted()) more").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .dropDestination(for: String.self) { ids, _ in
                    store.enqueue(ids)
                    return true
                }
            }
        }
    }
}

private struct QueueRow: View {
    @Environment(Store.self) private var store
    let entry: QueueEntry
    let index: Int
    @State private var hover = false

    var body: some View {
        HStack(spacing: 10) {
            Cover(path: entry.artwork, seed: entry.album + entry.artist, side: 32, corner: 4)
            VStack(alignment: .leading, spacing: 1) {
                Text(entry.title).lineLimit(1)
                Text(entry.artist).font(.caption).foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            Spacer()
            if hover {
                Button { store.core.moveUpNext(from: UInt32(index), to: UInt32(max(0, index - 1))) } label: { Image(systemName: "arrow.up") }
                    .buttonStyle(.borderless).disabled(index == 0).help("Move up")
                Button { store.core.removeUpNext(index: UInt32(index)) } label: { Image(systemName: "xmark") }
                    .buttonStyle(.borderless).help("Remove")
            } else {
                Text(clock(entry.duration)).font(.caption).monospacedDigit().foregroundStyle(store.colors.ink3)
            }
        }
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(count: 2) { store.core.jumpTo(id: entry.id, index: UInt32(index)) }
        .help(entry.reason)
        .contextMenu {
            Button("Play Now") { store.core.jumpTo(id: entry.id, index: UInt32(index)) }
            Button("Remove") { store.core.removeUpNext(index: UInt32(index)) }
        }
    }
}
