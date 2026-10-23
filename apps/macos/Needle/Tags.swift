import SwiftUI

/// Edits the tags of one song or many: only the fields changed are written. Each file is
/// backed up first and checked so its audio stays identical; earlier versions can be put back.
struct TagSheet: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let ids: [String]

    struct Target: Identifiable {
        let ids: [String]
        var id: String { ids.joined(separator: ",") }
    }

    private enum Field: String, CaseIterable {
        case title = "Title", artist = "Artist", album = "Album", albumArtist = "Album artist"
        case genre = "Genre", year = "Year", trackNumber = "Track number"
        var key: String {
            switch self {
            case .title: "title"
            case .artist: "artist"
            case .album: "album"
            case .albumArtist: "album_artist"
            case .genre: "genre"
            case .year: "year"
            case .trackNumber: "track_number"
            }
        }
    }

    @State private var common: CommonTags?
    @State private var values: [Field: String] = [:]
    @State private var original: [Field: String] = [:]
    @State private var mixed: Set<Field> = []
    @State private var saving = false
    @State private var progress: TagProgress?
    @State private var backups: [TagBackupInfo] = []
    @State private var streamed = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(ids.count == 1 ? "Edit tags" : "Edit tags on \(ids.count) tracks").font(.title2.weight(.semibold))
            if streamed {
                Text("Songs on a music server cannot be edited here. Change them on the server, or save them to your music first.")
                    .foregroundStyle(store.colors.danger)
            }
            Form {
                ForEach(Field.allCases, id: \.self) { field in
                    TextField(field.rawValue, text: Binding(get: { values[field] ?? "" }, set: { values[field] = $0 }),
                              prompt: Text(mixed.contains(field) ? "Mixed · leave empty to keep" : ""))
                }
            }
            .formStyle(.grouped)
            .disabled(saving || streamed)
            Text(ids.count == 1 ? "Saving writes to the file. Needle keeps a backup and checks the audio is unchanged."
                                : "Only fields you change are written. Each file is backed up and checked so its audio stays identical.")
                .font(.caption).foregroundStyle(.secondary)
            if let progress {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Writing tags and checking the audio is unchanged… \(progress.done) of \(progress.total)").font(.caption)
                    ProgressView(value: Double(progress.done), total: Double(max(1, progress.total)))
                }
            }
            if ids.count == 1 && !backups.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Earlier versions of this file").font(.callout.weight(.medium))
                    ForEach(backups, id: \.createdAt) { backup in
                        HStack {
                            Text(Date(timeIntervalSince1970: TimeInterval(backup.createdAt)).formatted(date: .abbreviated, time: .shortened))
                            Text(ByteCountFormatter.string(fromByteCount: Int64(backup.size), countStyle: .file)).foregroundStyle(.secondary)
                            Spacer()
                            Button("Restore") { restore(backup) }
                                .help("Put this version back. The current file is backed up first, so this can be undone too.")
                        }
                        .font(.callout)
                    }
                }
            }
            HStack {
                Spacer()
                if saving {
                    Button("Stop") { store.core.stopTags() }
                } else {
                    Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                }
                Button(ids.count == 1 ? "Save to file" : "Save to files") { save() }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .disabled(saving || streamed)
            }
        }
        .padding(22)
        .frame(width: 520)
        .task { load() }
    }

    private func load() {
        guard let found = JSON.decode(CommonTags.self, try? store.core.commonTagsJson(ids: ids)) else { return }
        common = found
        let pairs: [(Field, CommonTags.Common<String>?, CommonTags.Common<Int>?)] = [
            (.title, found.title, nil), (.artist, found.artist, nil), (.album, found.album, nil),
            (.albumArtist, found.albumArtist, nil), (.genre, found.genre, nil), (.year, nil, found.year),
            (.trackNumber, nil, found.trackNumber),
        ]
        for (field, text, number) in pairs {
            if let text {
                if text.state == "same" { values[field] = text.value ?? "" } else { mixed.insert(field) }
            } else if let number {
                if number.state == "same" { values[field] = (number.value ?? 0) > 0 ? String(number.value!) : "" } else { mixed.insert(field) }
            }
        }
        original = values
        if let tracks = JSON.decode([Track].self, try? store.core.tracksJson(ids: ids)) {
            streamed = tracks.contains(where: \.streamed)
        }
        if ids.count == 1 { backups = (try? store.core.tagBackups(id: ids[0])) ?? [] }
    }

    private func save() {
        var edit: [String: Any] = [:]
        for field in Field.allCases {
            let now = (values[field] ?? "").trimmingCharacters(in: .whitespaces)
            let before = (original[field] ?? "").trimmingCharacters(in: .whitespaces)
            // Mixed fields left empty keep each song's own value.
            if mixed.contains(field) && now.isEmpty { continue }
            if now == before && !mixed.contains(field) { continue }
            switch field {
            case .year, .trackNumber:
                guard now.isEmpty || UInt32(now) != nil else { store.fail("\(field.rawValue) needs a whole number."); return }
                edit[field.key] = UInt32(now) ?? 0
            default:
                edit[field.key] = now
            }
        }
        guard !edit.isEmpty else { store.notify("Nothing changed."); dismiss(); return }
        let json = JSON.text(edit)
        let ids = ids
        saving = true
        let watch = Task {
            while !Task.isCancelled {
                progress = store.core.tagsProgress()
                try? await Task.sleep(for: .milliseconds(200))
            }
        }
        Task {
            let report = await store.ask { try $0.writeTagsJson(ids: ids, edit: json) }
            watch.cancel()
            saving = false
            progress = nil
            guard let report else { return }
            if report.failed.isEmpty {
                store.notify("Saved tags to \(report.saved) files. Originals are backed up in your library folder.")
            } else {
                store.fail("Saved \(report.saved) files. \(report.failed.count) could not be changed, for example \(report.failed[0])")
            }
            store.libraryChanged()
            dismiss()
        }
    }

    private func restore(_ backup: TagBackupInfo) {
        let id = ids[0]
        Task {
            if await store.ask({ try $0.restoreTags(id: id, createdAt: backup.createdAt) }) != nil {
                store.notify("Earlier version restored. The replaced file is kept as a backup.")
                store.libraryChanged()
                dismiss()
            }
        }
    }
}
