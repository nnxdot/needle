import SwiftUI

/// Fix my library: songs you have more than once, albums without covers, albums missing tags,
/// and files to tidy into folders by a pattern.
struct DoctorPage: View {
    private var store: Store { .shared }
    @State private var tab = "Duplicates"

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Fix my library").font(store.display(28))
                Text("Find copies, fill in covers and tags, and tidy your files.").foregroundStyle(store.colors.ink2)
            }
            Picker("", selection: $tab) {
                ForEach(["Duplicates", "Covers", "Tags", "Organize"], id: \.self) { Text($0).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 420)
            switch tab {
            case "Covers": CoversTab()
            case "Tags": TagsTab()
            case "Organize": OrganizeTab()
            default: DuplicatesTab()
            }
        }
        .padding(28)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

private struct DuplicatesTab: View {
    private var store: Store { .shared }
    @State private var sets: [DuplicateSet]?
    @State private var confirmAll = false
    /// A merge is under way: the buttons wait for it.
    @State private var merging = false

    var body: some View {
        Group {
            if let sets {
                if sets.isEmpty {
                    done("No duplicates", "Every song is in your library once.")
                } else {
                    VStack(alignment: .leading, spacing: 10) {
                        HStack {
                            Text("\(sets.count == 1 ? "1 song is" : "\(sets.count) songs are") in your library more than once. Needle keeps the best copy of each; the others' plays, stars, and playlist places move to it, and their files go to the Trash.")
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button("Merge all (\(sets.reduce(0) { $0 + $1.songs.count - 1 }))") { confirmAll = true }.disabled(merging)
                        }
                        List(Array(sets.enumerated()), id: \.offset) { _, group in
                            VStack(alignment: .leading, spacing: 4) {
                                ForEach(Array(group.songs.enumerated()), id: \.offset) { i, song in
                                    HStack {
                                        Image(systemName: i == Int(group.keep) ? "checkmark.circle.fill" : "circle")
                                            .foregroundStyle(i == Int(group.keep) ? store.colors.accent : .secondary)
                                        Text(song.title).fontWeight(i == Int(group.keep) ? .semibold : .regular)
                                        Text("\(song.artist) · \(song.format)").foregroundStyle(.secondary)
                                        Spacer()
                                        Text(group.paths[i]).font(.caption).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle).frame(maxWidth: 300)
                                    }
                                }
                                Button("Merge") { merge(group) }.controlSize(.small).disabled(merging)
                            }
                            .padding(.vertical, 4)
                        }
                    }
                }
            } else {
                busy("Looking for songs you have more than once…")
            }
        }
        .task { sets = await store.load { $0.duplicates() } }
        .confirmationDialog("Merge every duplicate?", isPresented: $confirmAll) {
            // One after another: each changes the playlists the next one reads.
            Button("Merge all") {
                guard !merging else { return }
                merging = true
                let all = sets ?? []
                Task {
                    defer { merging = false }
                    var (merged, stuck) = (0, 0)
                    for group in all {
                        let result = await self.merged(group, quiet: true)
                        merged += result.merged
                        stuck += result.stuck
                    }
                    store.notify(stuck == 0 ? "Merged \(merged == 1 ? "1 copy" : "\(merged) copies"). They are in the Trash."
                                 : "Merged \(merged == 1 ? "1 copy" : "\(merged) copies"). \(stuck == 1 ? "1 copy" : "\(stuck) copies") could not go to the Trash and stayed.")
                }
            }
        } message: { Text("The extra copies go to the Trash.") }
    }

    private func merge(_ group: DuplicateSet) {
        guard !merging else { return }
        merging = true
        Task {
            _ = await merged(group, quiet: false)
            merging = false
        }
    }

    /// Merges a group; how many copies went, and how many stayed (their files would not go to
    /// the Trash). Quiet: the caller says it, once for many groups.
    private func merged(_ group: DuplicateSet, quiet: Bool) async -> (merged: Int, stuck: Int) {
        let keep = Int(group.keep)
        let extra = group.songs.indices.filter { $0 != keep }
        // The extra copies' files to the Trash, as the Finder does (Put Back works); a copy
        // whose file is already gone only leaves the library.
        // Away from the window: many copies, or a slow drive, would hold it still.
        let paths = extra.map { i in i < group.paths.count ? group.paths[i] : nil }
        let results = await Task.detached { () -> [String?] in
            paths.map { path -> String? in
                guard let path else { return "Its file is not known" }
                let url = URL(fileURLWithPath: path)
                guard FileManager.default.fileExists(atPath: url.path) else { return nil }
                do { try FileManager.default.trashItem(at: url, resultingItemURL: nil); return nil } catch { return error.localizedDescription }
            }
        }.value
        var gone: [Int] = []
        for (n, i) in extra.enumerated() {
            if let problem = results[n] {
                // This copy stays as it is, in the library and on disk; the rest are merged.
                if !quiet { store.fail("\(URL(fileURLWithPath: group.paths[i]).lastPathComponent) could not go to the Trash: \(problem)") }
            } else {
                gone.append(i)
            }
        }
        let stuck = extra.count - gone.count
        guard !gone.isEmpty else { return (0, stuck) }
        let (id, others) = (group.songs[keep].id, gone.map { group.songs[$0].id })
        // Not merged: their files are in the Trash all the same (the next scan marks them gone).
        guard await store.ask({ try $0.mergeInto(keep: id, others: others) }) != nil else { return (0, stuck) }
        if stuck == 0 {
            sets?.removeAll { $0.songs.first?.id == group.songs.first?.id }
        } else if let at = sets?.firstIndex(where: { $0.songs.first?.id == group.songs.first?.id }) {
            // Still a group: the kept song and the copies that stayed.
            let left = [keep] + extra.filter { !gone.contains($0) }
            var smaller = group
            smaller.songs = left.map { group.songs[$0] }
            smaller.paths = left.map { $0 < group.paths.count ? group.paths[$0] : "" }
            smaller.keep = 0
            sets?[at] = smaller
        }
        if !quiet, stuck == 0 {
            store.notify(others.count == 1 ? "Merged 1 copy. It is in the Trash." : "Merged \(others.count) copies. They are in the Trash.")
        }
        store.libraryChanged()
        return (others.count, stuck)
    }
}

private struct CoversTab: View {
    private var store: Store { .shared }
    @State private var albums: [Song]?
    @State private var looking = false

    var body: some View {
        Group {
            if let albums {
                if albums.isEmpty {
                    done("Every album has a cover", "")
                } else {
                    VStack(alignment: .leading, spacing: 10) {
                        HStack {
                            Text("\(count(albums.count, "album")) have no cover.").foregroundStyle(.secondary)
                            Spacer()
                            if !store.settings.onlineMedia {
                                Button("Turn on online lookups") { store.set(["online_media": true]) }
                            }
                            Button(looking ? "Looking…" : "Find \(albums.count) online") { find(albums) }
                                .disabled(looking || !store.settings.onlineMedia)
                        }
                        List(albums, id: \.id) { song in
                            HStack {
                                Cover(path: nil, seed: song.album + song.artist, side: 32, corner: 4)
                                Text(song.album.isEmpty ? "Unknown album" : song.album)
                                Text(song.artist).foregroundStyle(.secondary)
                                Spacer()
                                Button("Look up") { find([song]) }.controlSize(.small).disabled(looking || !store.settings.onlineMedia)
                            }
                        }
                    }
                }
            } else {
                busy("Looking for albums without covers…")
            }
        }
        .task { albums = await store.load { $0.albumsWithoutCovers() } }
    }

    private func find(_ list: [Song]) {
        looking = true
        let ids = list.map(\.id)
        Task {
            let found = await store.load { $0.findCovers(ids: ids) } ?? 0
            looking = false
            store.notify("Found \(found) of \(list.count).")
            albums = await store.load { $0.albumsWithoutCovers() }
            store.libraryChanged()
        }
    }
}

private struct TagsTab: View {
    private var store: Store { .shared }
    @State private var problems: [AlbumProblem]?
    @State private var open: AlbumProblem?
    @State private var releases: [ReleaseChoice]?

    var body: some View {
        Group {
            if let open {
                VStack(alignment: .leading, spacing: 10) {
                    Button { self.open = nil; releases = nil } label: { Label("Back to the list", systemImage: "chevron.left") }.buttonStyle(.link)
                    Text("\(open.album) · \(open.artist)").font(.title3.weight(.semibold))
                    Text("Pick the release that is your copy").foregroundStyle(.secondary)
                    if let releases {
                        if releases.isEmpty { Text("MusicBrainz has no release that matches.").foregroundStyle(.secondary) }
                        List(releases, id: \.id) { r in
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(r.title).fontWeight(.medium)
                                    Text([r.artist, r.year > 0 ? String(r.year) : nil, r.country, r.format, "\(r.matched) of \(r.tracks) tracks match"]
                                        .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                                        .font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                Button("Write tags") { apply(r) }.controlSize(.small)
                            }
                        }
                    } else {
                        busy("Searching MusicBrainz…")
                    }
                }
            } else if let problems {
                if problems.isEmpty {
                    done("Album tags look complete", "Every album has a year, track numbers, and an album artist where needed.")
                } else {
                    List(problems, id: \.album) { p in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(p.album.isEmpty ? "Unknown album" : p.album).fontWeight(.medium)
                                Text("\(p.artist) · \(p.problems.joined(separator: ", "))").font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Look up") { lookUp(p) }.controlSize(.small)
                        }
                    }
                }
            } else {
                busy("Checking album tags…")
            }
        }
        .task { problems = await store.load { $0.albumProblems() } }
    }

    private func lookUp(_ p: AlbumProblem) {
        open = p
        releases = nil
        let ids = p.songIds
        Task { releases = await store.ask { try $0.findReleases(ids: ids) } ?? [] }
    }

    private func apply(_ r: ReleaseChoice) {
        let id = r.id
        Task {
            guard let report = await store.ask({ try $0.applyRelease(releaseId: id) }) else { return }
            if report.failed.isEmpty {
                store.notify("Updated \(report.saved) of “\(open?.album ?? r.title)”.")
            } else {
                store.fail("Updated \(report.saved); \(report.failed.count) failed: \(report.failed[0])")
            }
            open = nil
            problems = await store.load { $0.albumProblems() }
            store.libraryChanged()
        }
    }
}

private struct OrganizeTab: View {
    private var store: Store { .shared }
    @State private var pattern = ""
    @State private var patterns: [String] = []
    @State private var plan: TidyPlan?
    @State private var confirm = false
    @State private var working = false
    /// A plan being worked out: the one shown cannot be moved meanwhile, since the new one
    /// takes its place in the core.
    @State private var previewing = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Rename and move song files inside your music folders by a pattern. Words in braces are filled in from each song's tags: {album_artist} {artist} {album} {title} {year} {track} {disc} {disc-}. Lyrics files move with their songs, and emptied folders are removed.")
                .foregroundStyle(.secondary)
            HStack {
                TextField("Pattern", text: $pattern).textFieldStyle(.roundedBorder).font(.body.monospaced())
                Menu("Patterns") { ForEach(patterns, id: \.self) { p in Button(p) { pattern = p } } }.fixedSize()
                Button(previewing ? "Working it out…" : "Preview") { preview() }.disabled(previewing || working)
            }
            if let plan {
                VStack(alignment: .leading, spacing: 6) {
                    Text("\(count(Int(plan.moves), "file")) to move · \(plan.inPlace) already in place · \(plan.skipped) left alone")
                    ForEach(plan.examples, id: \.self) { Text($0).font(.caption.monospaced()).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle) }
                    Button(working ? "Working out where each file goes…" : "Move files") { confirm = true }
                        .buttonStyle(.borderedProminent)
                        .disabled(plan.moves == 0 || working || previewing)
                }
                .padding(12)
                .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 10))
            }
            if store.core.canUndoTidy() {
                Button("Undo last move") {
                    store.job { "Moved \(try $0.undoTidy()) back." }
                }
            }
            Spacer()
        }
        .task {
            patterns = store.core.tidyPatterns()
            if pattern.isEmpty { pattern = patterns.first ?? "" }
        }
        .confirmationDialog("Move \(plan.map { count(Int($0.moves), "file") } ?? "the files")?", isPresented: $confirm) {
            Button("Move files") { move() }
        } message: { Text("You can undo this.") }
    }

    private func preview() {
        let p = pattern
        previewing = true
        plan = nil
        Task {
            plan = await store.load { $0.planTidy(pattern: p) }
            previewing = false
        }
    }

    private func move() {
        working = true
        Task {
            if let result = await store.ask({ try $0.tidy() }) {
                if result.failures.isEmpty {
                    store.notify("Moved \(count(Int(result.moved), "file")). You can undo this.")
                } else {
                    store.fail("Moved \(result.moved); \(result.failures.count) could not move. \(result.failures[0].path): \(result.failures[0].reason)")
                }
                store.libraryChanged()
            }
            working = false
            plan = nil
        }
    }
}

private func busy(_ text: String) -> some View {
    HStack(spacing: 10) {
        ProgressView().controlSize(.small)
        Text(text).foregroundStyle(.secondary)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
}

private func done(_ title: String, _ detail: String) -> some View {
    VStack(spacing: 8) {
        Image(systemName: "checkmark.seal").font(.system(size: 34)).foregroundStyle(.green)
        Text(title).font(.title3.weight(.medium))
        if !detail.isEmpty { Text(detail).foregroundStyle(.secondary) }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
}
