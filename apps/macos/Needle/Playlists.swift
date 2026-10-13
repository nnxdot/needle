import SwiftUI
import UniformTypeIdentifiers

/// Makes or changes a playlist: songs you pick, or a smart one whose songs match rules (built
/// with menus, or typed in the search's rule language).
struct PlaylistEditor: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let playlist: PlaylistModel
    @State private var name = ""
    @State private var description = ""
    @State private var smart = false
    @State private var typed = false
    @State private var rule = ""
    @State private var builder = RuleSet(any: false, rules: [], order: "Library order", limit: nil)
    @State private var kinds: RuleKinds?
    @State private var matching: Int?
    @State private var problem: String?
    @State private var picture: String?
    @State private var clearPicture = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(playlist.id.isEmpty ? "New playlist" : "Edit playlist").font(.title2.weight(.semibold))
            HStack(alignment: .top, spacing: 16) {
                VStack(spacing: 6) {
                    Group {
                        if let picture {
                            Artwork(path: picture, seed: name, size: 240)
                        } else if !clearPicture, let cover = playlist.cover {
                            Artwork(path: cover, seed: name, size: 240)
                        } else {
                            Placeholder(seed: name.isEmpty ? "playlist" : name)
                        }
                    }
                    .frame(width: 120, height: 120)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                    Button("Choose picture…") { choosePicture() }.controlSize(.small)
                    if picture != nil || (playlist.cover != nil && !clearPicture) {
                        Button("Use song covers") { picture = nil; clearPicture = true }.controlSize(.small).buttonStyle(.link)
                    }
                }
                VStack(alignment: .leading, spacing: 8) {
                    TextField("Name your playlist", text: $name).textFieldStyle(.roundedBorder).font(.title3)
                    TextField("Add a description (optional)", text: $description, axis: .vertical)
                        .textFieldStyle(.roundedBorder).lineLimit(2...3)
                    Picker("", selection: $smart) {
                        Text("Songs I pick").tag(false)
                        Text("Songs that match").tag(true)
                    }
                    .pickerStyle(.segmented).labelsHidden()
                    .disabled(!playlist.id.isEmpty)
                }
            }
            if smart { rules } else {
                Text(playlist.trackIds.isEmpty
                     ? "An empty playlist. Add songs from any song's menu (Add to Playlist), or drag them onto it in the sidebar."
                     : "\(count(playlist.trackIds.count, "song")) to start with.")
                    .font(.callout).foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
            HStack {
                if let problem { Text(problem).font(.caption).foregroundStyle(store.colors.danger).lineLimit(2) }
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button(playlist.id.isEmpty ? "Make playlist" : "Save") { save() }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(22)
        .frame(width: 640, height: smart ? 620 : 380)
        .task {
            name = playlist.name
            description = playlist.description
            smart = playlist.query != nil
            if let rules = playlist.rules { builder = rules } else if let q = playlist.query { rule = q; typed = true }
            kinds = JSON.decode(RuleKinds.self, store.core.ruleKindsJson(), plain: true)
            if builder.rules.isEmpty && !typed { builder.rules = [Rule(field: "Artist", op: "contains", value: "")] }
        }
        .task(id: "\(smart)\(typed)\(rule)\(builder.hashValue)") {
            try? await Task.sleep(for: .milliseconds(250))
            preview()
        }
    }

    @ViewBuilder
    private var rules: some View {
        VStack(alignment: .leading, spacing: 10) {
            if typed {
                Text("A rule typed by hand, in the same language as the search box.").font(.callout).foregroundStyle(.secondary)
                TextField("Type a rule, such as  rating >= 4", text: $rule).textFieldStyle(.roundedBorder).font(.body.monospaced())
                Button("Build it with menus instead") { typed = false; if builder.rules.isEmpty { builder.rules = [Rule(field: "Artist", op: "contains", value: "")] } }
                    .buttonStyle(.link)
            } else if let kinds {
                HStack {
                    Text("Songs that match")
                    Picker("", selection: $builder.any) {
                        Text("all of these").tag(false)
                        Text("any of these").tag(true)
                    }
                    .labelsHidden().fixedSize()
                }
                ScrollView {
                    VStack(spacing: 6) {
                        ForEach($builder.rules) { $r in
                            RuleRow(rule: $r, kinds: kinds.kinds) { builder.rules.removeAll { $0.id == r.id } }
                        }
                    }
                }
                .frame(maxHeight: 200)
                Button { builder.rules.append(Rule(field: "Artist", op: "contains", value: "")) } label: { Label("Add a rule", systemImage: "plus") }
                HStack {
                    Picker("Order", selection: $builder.order) {
                        ForEach(kinds.orders, id: \.self) { Text($0).tag($0) }
                    }
                    .fixedSize()
                    Picker("At most", selection: Binding(get: { builder.limit.map(Int.init) ?? 0 }, set: { builder.limit = $0 == 0 ? nil : UInt32($0) })) {
                        Text("No limit").tag(0)
                        ForEach([10, 25, 50, 100, 250, 500], id: \.self) { Text("\($0) songs").tag($0) }
                    }
                    .fixedSize()
                }
                Button("Type the rule instead") {
                    rule = (try? store.core.rulesQuery(json: rulesJSON)) ?? ""
                    typed = true
                }
                .buttonStyle(.link)
            }
            Text(matching.map { $0 == 0 ? "No songs match." : "\(count($0, "song")) match now." } ?? "Counting…")
                .font(.callout).foregroundStyle(.secondary)
        }
    }

    private var rulesJSON: String {
        String(decoding: (try? JSONEncoder().encode(builder)) ?? Data(), as: UTF8.self)
    }

    private func currentRule() throws -> String {
        typed ? rule.trimmingCharacters(in: .whitespaces) : try store.core.rulesQuery(json: rulesJSON)
    }

    private func preview() {
        guard smart else { return }
        do {
            let q = try currentRule()
            if let error = store.core.checkQuery(query: q).error {
                problem = error
                matching = nil
                return
            }
            problem = nil
            matching = Int((try? store.core.countMatching(query: q)) ?? 0)
        } catch {
            problem = error.message
            matching = nil
        }
    }

    private func choosePicture() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        panel.message = "Choose a picture for the playlist"
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        picture = path
        clearPicture = false
    }

    private func save() {
        var next = playlist
        next.name = name
        next.description = description
        if smart {
            if typed {
                next.query = rule.trimmingCharacters(in: .whitespaces)
                next.rules = nil
            } else {
                next.rules = builder
                next.query = (try? store.core.rulesQuery(json: rulesJSON)) ?? ""
            }
            next.trackIds = []
        } else {
            next.query = nil
            next.rules = nil
        }
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        guard let data = try? encoder.encode(next) else { return }
        let json = String(decoding: data, as: UTF8.self)
        let (picture, clear, isNew, title) = (picture, clearPicture, playlist.id.isEmpty, name)
        Task {
            guard let id = await store.ask({ try $0.savePlaylistJson(json: json) }) else { return }
            if picture != nil || clear {
                _ = await store.ask { try $0.setPlaylistPicture(id: id, path: picture) }
            }
            store.notify(isNew ? "Made “\(title)”." : "Saved “\(title)”.")
            store.libraryChanged()
            if isNew { store.navigate(.playlist(id)) }
            dismiss()
        }
    }
}

/// One rule: what it looks at, how it compares, and the value.
private struct RuleRow: View {
    @Binding var rule: Rule
    let kinds: [RuleKinds.Kind]
    let remove: () -> Void

    var body: some View {
        let kind = kinds.first { $0.label == rule.field } ?? kinds.first
        HStack {
            Picker("", selection: Binding(get: { rule.field }, set: { field in
                rule.field = field
                if let k = kinds.first(where: { $0.label == field }), !k.ops.contains(rule.op) { rule.op = k.ops.first ?? "" }
            })) {
                ForEach(kinds, id: \.label) { Text($0.label).tag($0.label) }
            }
            .labelsHidden().frame(width: 150)
            Picker("", selection: $rule.op) {
                ForEach(kind?.ops ?? [], id: \.self) { Text($0).tag($0) }
            }
            .labelsHidden().frame(width: 140)
            if kind?.value != "none" {
                TextField(kind?.placeholder ?? "", text: $rule.value).textFieldStyle(.roundedBorder)
            } else {
                Spacer()
            }
            Button(action: remove) { Image(systemName: "minus.circle") }.buttonStyle(.borderless).help("Remove this rule")
        }
    }
}

/// Adds songs to a playlist of picked songs: search, and add what matches.
struct AddSongs: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let playlist: PlaylistModel
    @State private var text = ""
    @State private var found: [Track] = []
    @State private var added: Set<String> = []

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Add songs to “\(playlist.name)”").font(.title2.weight(.semibold))
            TextField("Search your songs, or type a rule", text: $text).textFieldStyle(.roundedBorder)
            List(found) { track in
                HStack(spacing: 10) {
                    Cover(path: track.artwork, seed: track.album + track.displayAlbumArtist, side: 32, corner: 4)
                    VStack(alignment: .leading) {
                        Text(track.title).lineLimit(1)
                        Text("\(track.displayArtist) · \(track.displayAlbum)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    Spacer()
                    if added.contains(track.id) || playlist.trackIds.contains(track.id) {
                        Image(systemName: "checkmark").foregroundStyle(store.colors.accent)
                    } else {
                        Button("Add to this playlist") { add([track.id]) }.controlSize(.small)
                    }
                }
            }
            .listStyle(.inset)
            HStack {
                Text(found.isEmpty ? (text.isEmpty ? "" : "No songs match.") : "\(count(found.count, "song")) shown").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Add all \(found.count)") { add(found.map(\.id)) }.disabled(found.isEmpty)
                Button("Done") { dismiss() }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 620, height: 560)
        .task(id: text) {
            try? await Task.sleep(for: .milliseconds(200))
            guard !Task.isCancelled else { return }
            let q = text.trimmingCharacters(in: .whitespaces)
            guard !q.isEmpty else { found = []; return }
            let json = await store.load { try $0.viewPage(query: q, playlist: nil, filter: "", offset: 0, size: 300) }
            guard !Task.isCancelled else { return }
            found = JSON.decode(TracksPage.self, json)?.tracks ?? []
        }
    }

    private func add(_ ids: [String]) {
        let id = playlist.id
        Task {
            if let n = await store.ask({ try $0.addSongsToPlaylist(id: id, ids: ids) }) {
                added.formUnion(ids)
                store.notify(n == 0 ? "Already in “\(playlist.name)”." : "Added \(count(Int(n), "track")) to “\(playlist.name)”.")
                store.libraryChanged()
            }
        }
    }
}
