import SwiftUI
import UniformTypeIdentifiers

/// Fix my library: duplicates, albums without covers, albums with missing tags, and tidying
/// the files into folders.
struct FixLibraryView: View {
    var body: some View {
        List {
            Section {
                NavigationLink(value: Route.duplicates) { Label("Duplicates", systemImage: "square.on.square") }
                NavigationLink(value: Route.missingCovers) { Label("Albums without covers", systemImage: "photo") }
                NavigationLink(value: Route.albumProblems) { Label("Albums with missing tags", systemImage: "tag") }
                NavigationLink(value: Route.tidy) { Label("Tidy the files", systemImage: "folder") }
            } footer: {
                Text("Every file is backed up before Needle changes its tags, and its sound is checked after. A song's menu has Edit Tags and Identify.")
            }
        }
        .navigationTitle("Fix my library")
        .themed()
    }
}

struct DuplicatesView: View {
    @Environment(Store.self) private var store
    @State private var sets: [DuplicateSet]?
    @State private var trash: [String] = []

    var body: some View {
        List {
            ForEach(Array((sets ?? []).enumerated()), id: \.offset) { n, set in
                Section {
                    ForEach(Array(set.songs.enumerated()), id: \.offset) { i, song in
                        Button {
                            sets?[n].keep = UInt32(i)
                        } label: {
                            HStack {
                                Image(systemName: Int(set.keep) == i ? "checkmark.circle.fill" : "circle").foregroundStyle(.tint)
                                VStack(alignment: .leading) {
                                    Text(song.title)
                                    Text("\(song.format) · \(clock(song.duration)) · \((set.paths[safe: i] as NSString?)?.lastPathComponent ?? "")")
                                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                                }
                            }
                        }
                        .buttonStyle(.plain)
                    }
                    Button("Keep the chosen one") { merge(set) }
                } header: {
                    Text(set.songs.first.map { "\($0.title) · \($0.artist)" } ?? "")
                }
            }
        }
        .overlay {
            if sets == nil { ProgressView("Looking for duplicates…") }
            else if sets?.isEmpty == true { ContentUnavailableView("No duplicates", systemImage: "checkmark.circle") }
        }
        .navigationTitle("Duplicates")
        .themed()
        .confirmationDialog("Delete the other files?", isPresented: Binding(get: { !trash.isEmpty }, set: { if !$0 { trash = [] } }), titleVisibility: .visible) {
            Button("Delete \(count(trash.count, "file"))", role: .destructive) {
                for path in trash { try? FileManager.default.removeItem(atPath: path) }
                trash = []
            }
            Button("Keep the files", role: .cancel) { trash = [] }
        } message: {
            Text("Their plays, stars, and playlist places moved to the one you kept.")
        }
        .task { sets = await store.load { $0.duplicates() } }
    }

    private func merge(_ set: DuplicateSet) {
        let keep = Int(set.keep)
        let others = set.songs.enumerated().filter { $0.offset != keep }.map(\.element.id)
        let paths = set.paths.enumerated().filter { $0.offset != keep }.map(\.element)
            .filter { $0.hasPrefix(NSHomeDirectory()) }
        let keepId = set.songs[keep].id
        Task {
            if await store.ask({ try $0.mergeDuplicates(keep: keepId, others: others) }) != nil {
                sets = await store.load { $0.duplicates() }
                store.libraryChanged()
                trash = paths
            }
        }
    }
}

extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}

struct MissingCoversView: View {
    @Environment(Store.self) private var store
    @State private var songs: [Song]?
    @State private var busy = false

    var body: some View {
        List {
            if let songs, !songs.isEmpty {
                Section {
                    Button(busy ? "Looking…" : "Find covers online") {
                        busy = true
                        let ids = songs.map(\.id)
                        Task {
                            let found = await store.load { $0.findCovers(ids: ids) } ?? 0
                            busy = false
                            store.message = found == 0 ? "No covers found" : "Found \(count(Int(found), "cover"))"
                            self.songs = await store.load { $0.albumsWithoutCovers() }
                            store.libraryChanged()
                        }
                    }
                    .disabled(busy)
                } footer: {
                    Text("From the Cover Art Archive, by each album's name and artist.")
                }
                Section {
                    ForEach(songs) { song in
                        VStack(alignment: .leading) {
                            Text(song.album.isEmpty ? song.title : song.album)
                            Text(song.artist).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .overlay {
            if songs == nil { ProgressView() }
            else if songs?.isEmpty == true { ContentUnavailableView("Every album has a cover", systemImage: "checkmark.circle") }
        }
        .navigationTitle("Without covers")
        .themed()
        .task { songs = await store.load { $0.albumsWithoutCovers() } }
    }
}

struct AlbumProblemsView: View {
    @Environment(Store.self) private var store
    @State private var albums: [AlbumProblem]?
    @State private var fixing: AlbumProblem?

    var body: some View {
        List(Array((albums ?? []).enumerated()), id: \.offset) { _, album in
            Button { fixing = album } label: {
                VStack(alignment: .leading, spacing: 2) {
                    Text(album.album).foregroundStyle(.primary)
                    Text(album.artist).font(.subheadline).foregroundStyle(.secondary)
                    Text(album.problems.joined(separator: " · ")).font(.caption).foregroundStyle(.orange)
                }
            }
        }
        .overlay {
            if albums == nil { ProgressView() }
            else if albums?.isEmpty == true { ContentUnavailableView("No missing tags", systemImage: "checkmark.circle") }
        }
        .navigationTitle("Missing tags")
        .themed()
        .sheet(item: Binding(get: { fixing.map(Identified.init) }, set: { fixing = $0?.value })) { item in
            NavigationStack { ReleasesView(album: item.value) { changed in
                fixing = nil
                if changed { Task { albums = await store.load { $0.albumProblems() } } }
            } }
        }
        .task { albums = await store.load { $0.albumProblems() } }
    }
}

/// Wraps a value for `sheet(item:)`.
struct Identified<T>: Identifiable {
    let id = UUID()
    let value: T
}

/// Releases on MusicBrainz that may be an album: choose one to write its names and numbers.
struct ReleasesView: View {
    @Environment(Store.self) private var store
    let album: AlbumProblem
    let done: (Bool) -> Void
    @State private var choices: [ReleaseChoice]?

    var body: some View {
        List(choices ?? [], id: \.id) { release in
            Button {
                let id = release.id
                Task {
                    if let report = await store.ask({ try $0.applyRelease(releaseId: id) }) {
                        store.message = report.failed.isEmpty ? "Saved \(count(Int(report.saved), "song"))" : "Saved \(report.saved); \(report.failed.count) could not change"
                        store.libraryChanged()
                        done(true)
                    }
                }
            } label: {
                VStack(alignment: .leading, spacing: 2) {
                    Text(release.title).foregroundStyle(.primary)
                    Text(release.artist).font(.subheadline).foregroundStyle(.secondary)
                    Text([release.year > 0 ? String(release.year) : nil, release.country.isEmpty ? nil : release.country, release.format.isEmpty ? nil : release.format,
                          "\(release.matched) of \(release.tracks) songs found"].compactMap { $0 }.joined(separator: " · "))
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .overlay {
            if choices == nil { ProgressView("Asking MusicBrainz…") }
            else if choices?.isEmpty == true { ContentUnavailableView("Nothing found on MusicBrainz", systemImage: "magnifyingglass") }
        }
        .navigationTitle(album.album)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { done(false) } } }
        .task {
            let ids = album.songIds
            choices = await store.ask { try $0.findReleases(ids: ids) } ?? []
        }
    }
}

struct TidyView: View {
    @Environment(Store.self) private var store
    @State private var patterns: [String] = []
    @State private var pattern = ""
    @State private var plan: TidyPlan?
    @State private var canUndo = false
    @State private var result: TidyResult?

    var body: some View {
        Form {
            Section {
                Picker("Folders", selection: $pattern) {
                    ForEach(patterns, id: \.self) { Text($0).tag($0) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
            } header: {
                Text("Lay the files out as")
            } footer: {
                Text("Files move only inside Needle's folder. Lyrics files go with their songs.")
            }
            if let plan {
                Section("What happens") {
                    LabeledContent("Files that move", value: plan.moves.formatted())
                    LabeledContent("Already in place", value: plan.inPlace.formatted())
                    if plan.skipped > 0 { LabeledContent("Left as they are", value: plan.skipped.formatted()) }
                    ForEach(plan.examples, id: \.self) { Text($0).font(.caption).foregroundStyle(.secondary) }
                    Button("Move the files") {
                        Task {
                            result = await store.ask { try $0.tidy() }
                            canUndo = await store.load { $0.canUndoTidy() } ?? false
                            store.rescan()
                            await planAgain()
                        }
                    }
                    .disabled(plan.moves == 0)
                }
            }
            if let result {
                Section("Done") {
                    Text("Moved \(count(Int(result.moved), "file"))")
                    ForEach(result.failures, id: \.path) { f in
                        Text("\((f.path as NSString).lastPathComponent): \(f.reason)").font(.caption).foregroundStyle(.red)
                    }
                }
            }
            if canUndo {
                Section {
                    Button("Put the files back") {
                        Task {
                            if let back = await store.ask({ try $0.undoTidy() }) { store.message = "Put back \(count(Int(back), "file"))" }
                            canUndo = await store.load { $0.canUndoTidy() } ?? false
                            store.rescan()
                            await planAgain()
                        }
                    }
                }
            }
        }
        .navigationTitle("Tidy the files")
        .themed()
        .task {
            patterns = await store.load { $0.tidyPatterns() } ?? []
            if pattern.isEmpty { pattern = patterns.first ?? "" }
            canUndo = await store.load { $0.canUndoTidy() } ?? false
        }
        .task(id: pattern) { await planAgain() }
    }

    private func planAgain() async {
        guard !pattern.isEmpty else { return }
        let p = pattern
        plan = await store.load { $0.planTidy(pattern: p) }
    }
}

// MARK: - Tags

/// Edits the tags of one song or several (a field left empty stays as each song has it), and
/// puts back earlier versions of one song's tags.
struct TagEditorView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let ids: [String]
    @State private var fields = Fields()
    @State private var original = Fields()
    @State private var backups: [TagBackupInfo] = []

    struct Fields: Equatable {
        var title = "", artist = "", album = "", albumArtist = "", genre = "", year = "", track = ""
    }

    var body: some View {
        Form {
            Section {
                if ids.count == 1 { TextField("Title", text: $fields.title) }
                TextField("Artist", text: $fields.artist)
                TextField("Album", text: $fields.album)
                TextField("Album artist", text: $fields.albumArtist)
                TextField("Genre", text: $fields.genre)
                TextField("Year", text: $fields.year).keyboardType(.numberPad)
                if ids.count == 1 { TextField("Track number", text: $fields.track).keyboardType(.numberPad) }
            } footer: {
                Text(ids.count == 1 ? "The file is backed up first." : "\(ids.count) songs. A field left empty stays as each song has it. Each file is backed up first.")
            }
            if !backups.isEmpty {
                Section("Earlier versions") {
                    ForEach(backups, id: \.createdAt) { backup in
                        Button {
                            let (id, at) = (ids[0], backup.createdAt)
                            Task {
                                if await store.ask({ try $0.restoreTags(id: id, createdAt: at) }) != nil {
                                    store.message = "Tags put back"
                                    store.libraryChanged()
                                    dismiss()
                                }
                            }
                        } label: {
                            LabeledContent(Date(timeIntervalSince1970: TimeInterval(backup.createdAt)).formatted(date: .abbreviated, time: .shortened),
                                           value: ByteCountFormatter.string(fromByteCount: Int64(backup.size), countStyle: .file))
                        }
                    }
                }
            }
        }
        .navigationTitle(ids.count == 1 ? "Edit tags" : "Edit \(ids.count) songs")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            ToolbarItem(placement: .confirmationAction) { Button("Save") { save() }.disabled(fields == original) }
        }
        .task {
            guard ids.count == 1 else { return }
            let id = ids[0]
            if let tags = await store.load({ try $0.songTags(id: id) }) {
                fields = Fields(title: tags.title, artist: tags.artist, album: tags.album, albumArtist: tags.albumArtist, genre: tags.genre,
                                year: tags.year > 0 ? String(tags.year) : "", track: tags.trackNumber > 0 ? String(tags.trackNumber) : "")
                original = fields
            }
            backups = await store.load { try $0.tagBackups(id: id) } ?? []
        }
    }

    private func save() {
        let f = fields, o = original
        let one = ids.count == 1
        // Only what changed: with several songs, what was typed.
        func pick(_ new: String, _ old: String) -> String? { one ? (new == old ? nil : new) : (new.isEmpty ? nil : new) }
        let change = TagChange(title: pick(f.title, o.title), artist: pick(f.artist, o.artist), album: pick(f.album, o.album),
                               albumArtist: pick(f.albumArtist, o.albumArtist), genre: pick(f.genre, o.genre),
                               year: pick(f.year, o.year).flatMap { UInt32($0) ?? ($0.isEmpty ? 0 : nil) },
                               trackNumber: pick(f.track, o.track).flatMap { UInt32($0) ?? ($0.isEmpty ? 0 : nil) })
        let ids = ids
        Task {
            if let report = await store.ask({ try $0.editTags(ids: ids, change: change) }) {
                store.message = report.failed.isEmpty ? "Saved" : "Saved \(report.saved); \(report.failed.first ?? "")"
                store.libraryChanged()
                dismiss()
            }
        }
    }
}

/// What a song is, found from its sound with AcoustID and MusicBrainz.
struct IdentifyView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let id: String
    @State private var hasKey: Bool?
    @State private var key = ""
    @State private var matches: [SoundMatch]?

    var body: some View {
        Form {
            if hasKey == false {
                Section {
                    TextField("AcoustID key", text: $key).textInputAutocapitalization(.never).autocorrectionDisabled()
                    Button("Save key") {
                        let k = key
                        Task {
                            if await store.ask({ try $0.setAcoustidKey(key: k) }) != nil { hasKey = true; await find() }
                        }
                    }
                    .disabled(key.isEmpty)
                } footer: {
                    Text("Identifying a song by its sound needs a free key from acoustid.org/new-application.")
                }
            } else if let matches {
                if matches.isEmpty {
                    Text("No match for this song's sound.").foregroundStyle(.secondary)
                }
                ForEach(matches, id: \.recordingId) { match in
                    Button {
                        Task {
                            if await store.ask({ try $0.applyMatch(id: id, found: match) }) != nil {
                                store.message = "Saved"
                                store.libraryChanged()
                                dismiss()
                            }
                        }
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(match.title).foregroundStyle(.primary)
                            Text("\(match.artist) · \(match.album)").font(.subheadline).foregroundStyle(.secondary)
                            Text("\(match.score)% sure").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            } else {
                HStack { ProgressView(); Text("Listening to the song…") }
            }
        }
        .navigationTitle("Identify")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .task {
            hasKey = await store.load { $0.hasAcoustidKey() } ?? false
            if hasKey == true { await find() }
        }
    }

    private func find() async {
        let id = id
        matches = await store.ask { try $0.identify(id: id) } ?? []
    }
}

// MARK: - Import

struct ImportView: View {
    @Environment(Store.self) private var store
    @State private var kind: String?
    @State private var busy = false
    @State private var result: String?
    @State private var lastfmUser = ""
    @State private var lastfmKey = ""
    @State private var listenbrainzUser = ""

    var body: some View {
        Form {
            Section {
                card("Apple Music or iTunes", "A Library.xml file: plays, stars, and playlists.", "music.note", "itunes")
                card("Spotify", "Your data export, as a zip: listening history.", "waveform.circle", "spotify")
                card("A playlist file", "An .m3u or .m3u8 playlist.", "music.note.list", "playlist")
            } header: {
                Text("From another player")
            } footer: {
                Text("Songs are matched to the music in your library by name.")
            }
            Section("Last.fm history") {
                TextField("Last.fm user", text: $lastfmUser).textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("API key", text: $lastfmKey).textInputAutocapitalization(.never).autocorrectionDisabled()
                Button("Bring in") { history("lastfm", lastfmUser, lastfmKey) }.disabled(busy || lastfmUser.isEmpty || lastfmKey.isEmpty)
            }
            Section("ListenBrainz history") {
                TextField("ListenBrainz user", text: $listenbrainzUser).textInputAutocapitalization(.never).autocorrectionDisabled()
                Button("Bring in") { history("listenbrainz", listenbrainzUser, "") }.disabled(busy || listenbrainzUser.isEmpty)
            }
            if busy { HStack { ProgressView(); Text("Bringing it in…") } }
            if let result { Section("Done") { Text(result) } }
        }
        .navigationTitle("Import")
        .themed()
        .fileImporter(isPresented: Binding(get: { kind != nil }, set: { if !$0 { kind = nil } }),
                      allowedContentTypes: kind == "playlist" ? [UTType(filenameExtension: "m3u") ?? .plainText, UTType(filenameExtension: "m3u8") ?? .plainText, .plainText]
                                                              : kind == "spotify" ? [.zip, .folder] : [.xml]) { picked in
            let which = kind ?? ""
            kind = nil
            guard case .success(let url) = picked, let path = store.copyIn(url) else { return }
            busy = true
            Task {
                result = await store.ask { try $0.importFile(kind: which, path: path) }
                busy = false
                store.libraryChanged()
            }
        }
    }

    private func card(_ title: String, _ detail: String, _ symbol: String, _ which: String) -> some View {
        Button { kind = which } label: {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.title3).foregroundStyle(.tint).frame(width: 30)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).foregroundStyle(.primary)
                    Text(detail).font(.caption).foregroundStyle(.secondary)
                }
            }
        }
        .disabled(busy)
    }

    private func history(_ service: String, _ user: String, _ key: String) {
        busy = true
        Task {
            result = await store.ask { try $0.importHistory(service: service, user: user, apiKey: key) }
            busy = false
            store.libraryChanged()
        }
    }
}
