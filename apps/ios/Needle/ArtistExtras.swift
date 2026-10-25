import PhotosUI
import SwiftUI

/// An artist's three pictures, as the core names them.
enum PictureKind: String, CaseIterable, Identifiable {
    case photo, banner, logo
    var id: String { rawValue }
    var title: String {
        switch self {
        case .photo: "Photo"
        case .banner: "Wide picture"
        case .logo: "Logo"
        }
    }
}

/// The artist page's picture menu: for the photo, the wide picture, and the logo, Choose
/// Picture… (from Photos), Pick from fanart.tv…, and Reset (back to the one Needle found).
struct ArtistPicturesMenu: View {
    @Environment(Store.self) private var store
    let name: String
    let changed: () -> Void
    @State private var own: [String] = []
    @State private var choosing: PictureKind?
    @State private var item: PhotosPickerItem?
    @State private var picking: PictureKind?
    @State private var online = true

    var body: some View {
        Menu {
            ForEach(PictureKind.allCases) { kind in
                Section(kind.title) {
                    Button("Choose Picture…", systemImage: "photo") { choosing = kind }
                    Button("Pick from fanart.tv…", systemImage: "sparkles.rectangle.stack") { picking = kind }
                        .disabled(!online)
                    if own.contains(kind.rawValue) {
                        Button("Reset", systemImage: "arrow.uturn.backward") { reset(kind) }
                    }
                }
            }
        } label: {
            Image(systemName: "photo.on.rectangle")
        }
        .accessibilityLabel("The artist's pictures")
        .photosPicker(isPresented: Binding(get: { choosing != nil }, set: { if !$0 { choosing = nil } }),
                      selection: $item, matching: .images)
        .onChange(of: item) { _, chosen in
            guard let chosen, let kind = choosing else { return }
            item = nil
            Task { await use(chosen, as: kind) }
        }
        .sheet(item: $picking) { kind in
            NavigationStack {
                FanartPickerView(name: name, kind: kind) {
                    Task { await readOwn() }
                    changed()
                }
            }
        }
        .task(id: name) {
            await readOwn()
            online = await store.load { try $0.playbackSettings().onlineMedia } ?? true
        }
    }

    private func readOwn() async {
        let name = name
        own = await store.load { $0.artistOwnPictures(name: name) } ?? []
    }

    /// The chosen photo, through a file the core copies.
    private func use(_ chosen: PhotosPickerItem, as kind: PictureKind) async {
        defer { choosing = nil }
        guard let data = try? await chosen.loadTransferable(type: Data.self) else {
            store.message = "That picture could not be read"
            return
        }
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("artist-picture-\(UUID().uuidString)")
        do { try data.write(to: file) } catch { return }
        let (name, path) = (name, file.path)
        let done = await store.ask { try $0.setArtistPicture(name: name, kind: kind.rawValue, path: path) } != nil
        try? FileManager.default.removeItem(at: file)
        if done {
            await readOwn()
            changed()
        }
    }

    private func reset(_ kind: PictureKind) {
        let name = name
        Task {
            if await store.ask({ try $0.resetArtistPicture(name: name, kind: kind.rawValue) }) != nil {
                await readOwn()
                changed()
            }
        }
    }
}

/// All of fanart.tv's choices for one of an artist's pictures; a tap uses that one.
struct FanartPickerView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let name: String
    let kind: PictureKind
    let picked: () -> Void
    @State private var choices: [FanartPick]?
    @State private var busy: String?

    var body: some View {
        ScrollView {
            if let choices, choices.isEmpty {
                ContentUnavailableView("Nothing on fanart.tv", systemImage: "photo", description: Text("fanart.tv has no \(kind.title.lowercased()) of \(name)."))
            } else if let choices {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: kind == .photo ? 100 : 160), spacing: 10)], spacing: 10) {
                    ForEach(choices, id: \.url) { choice in
                        Button { pick(choice) } label: {
                            AsyncImage(url: URL(string: choice.preview)) { image in
                                image.resizable().scaledToFit()
                            } placeholder: {
                                Color.secondary.opacity(0.15)
                            }
                            .frame(height: kind == .photo ? 110 : 80)
                            .frame(maxWidth: .infinity)
                            .background(kind == .logo ? Color.black.opacity(0.6) : .clear)
                            .clipShape(.rect(cornerRadius: 8))
                            .overlay { if busy == choice.url { ProgressView() } }
                        }
                        .buttonStyle(Pressable())
                        .disabled(busy != nil)
                        .accessibilityLabel("A \(kind.title.lowercased()) from fanart.tv, \(choice.likes) likes")
                    }
                }
                .padding()
            } else {
                ProgressView().padding(.top, 80)
            }
        }
        .navigationTitle(kind.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        .task {
            let (name, kind) = (name, kind.rawValue)
            choices = await store.load { try $0.artistFanartChoices(name: name, kind: kind) } ?? []
        }
    }

    private func pick(_ choice: FanartPick) {
        busy = choice.url
        let (name, kind, url) = (name, kind.rawValue, choice.url)
        Task {
            let done = await store.ask { try $0.pickArtistFanart(name: name, kind: kind, url: url) } != nil
            busy = nil
            if done {
                picked()
                dismiss()
            }
        }
    }
}

/// Settings › Library › Artist pictures: the slow search for every artist's photo, logo, and
/// wide picture (on Wi-Fi only), with how far it is and Check all now.
struct ArtistPicturesRow: View {
    @Environment(Store.self) private var store
    @State private var progress: PictureProgress?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle(isOn: Binding(get: { store.more.artistPictures }, set: { on in store.updateMore { $0.artistPictures = on } })) {
                SettingLabel(title: "Artist pictures", detail: detail, icon: "person.crop.square.fill", tint: .pink)
            }
            if store.more.artistPictures, let progress, progress.total > 0, progress.done < progress.total {
                Button("Check all now") { store.core.artistPicturesCheckAllNow() }
                    .disabled(progress.hurry)
                    .font(.needle(.subheadline))
            }
        }
        .task {
            while !Task.isCancelled {
                progress = await store.load { $0.artistPicturesProgress() }
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    private var detail: String {
        guard store.more.artistPictures else { return "Photos, logos, and wide pictures for every artist, found slowly." }
        guard let progress, progress.total > 0 else { return "Finds every artist's photo, logo, and wide picture, one at a time." }
        if progress.done >= progress.total { return "All \(Int(progress.total).formatted()) artists looked for." }
        // On a phone it looks only on Wi-Fi.
        return "\(Int(progress.done).formatted()) of \(Int(progress.total).formatted()) artists looked for." + (progress.allowed ? "" : " Waits for Wi-Fi.")
    }
}

/// Every artist, in the order chosen (by name with an A–Z index at the side, most played,
/// most songs, or recently added), and only album artists when asked.
struct ArtistsView: View {
    @Environment(Store.self) private var store
    @State private var artists: [ListedArtist]?
    @State private var filter = ""

    var body: some View {
        let byName = store.more.artistOrder == "name" && filter.isEmpty
        List {
            if byName {
                ForEach(sections, id: \.0) { letter, group in
                    Section {
                        ForEach(group, id: \.name) { row($0) }
                    } header: {
                        Text(letter)
                    }
                    .sectionIndexLabel(letter)
                }
            } else {
                ForEach(shown, id: \.name) { row($0) }
            }
        }
        .listStyle(.plain)
        .listSectionIndexVisibility(byName ? .visible : .hidden)
        .overlay { if artists == nil { ProgressView() } else if artists?.isEmpty == true { ContentUnavailableView("No artists", systemImage: "music.microphone") } }
        .navigationTitle("Artists")
        .themed()
        .searchable(text: $filter, placement: .navigationBarDrawer, prompt: "Find in Artists")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Picker("Sort by", selection: Binding(get: { store.more.artistOrder }, set: { order in store.updateMore { $0.artistOrder = order } })) {
                        Text("A–Z").tag("name")
                        Text("Most played").tag("most_played")
                        Text("Most songs").tag("most_songs")
                        Text("Newest").tag("recently_added")
                    }
                    Toggle("Only album artists", isOn: Binding(get: { store.more.albumArtistsOnly }, set: { on in store.updateMore { $0.albumArtistsOnly = on } }))
                } label: {
                    Image(systemName: "arrow.up.arrow.down")
                }
                .accessibilityLabel("Sort artists")
            }
        }
        .task(id: "\(store.libraryVersion)|\(store.more.artistOrder)|\(store.more.albumArtistsOnly)") {
            artists = await store.load { try $0.listedArtists() } ?? []
        }
    }

    private func row(_ artist: ListedArtist) -> some View {
        let row = HStack(spacing: 14) {
            Artwork(path: artist.artwork, seed: artist.name, size: 160)
                .frame(width: 52, height: 52).clipShape(.circle)
            VStack(alignment: .leading, spacing: 2) {
                Text(isPlaceholderArtist(artist.name) ? unknownArtist : artist.name).lineLimit(1)
                Text(count(Int(artist.albums), "album") + " · " + count(Int(artist.songs), "song"))
                    .font(.needle(.subheadline)).foregroundStyle(.secondary)
            }
        }
        return Group {
            if isPlaceholderArtist(artist.name) {
                row
            } else {
                NavigationLink(value: Route.artist(artist.name)) { row }
            }
        }
    }

    private var shown: [ListedArtist] {
        let all = artists ?? []
        return filter.isEmpty ? all : all.filter { $0.name.localizedCaseInsensitiveContains(filter) }
    }

    /// The artists under each letter, in the order the core gives.
    private var sections: [(String, [ListedArtist])] {
        var order: [String] = []
        var groups: [String: [ListedArtist]] = [:]
        for artist in shown {
            let letter = artist.letter.isEmpty ? "#" : artist.letter
            if groups[letter] == nil { order.append(letter) }
            groups[letter, default: []].append(artist)
        }
        return order.map { ($0, groups[$0] ?? []) }
    }
}
