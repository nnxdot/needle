import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// One of fanart.tv's pictures of an artist, to choose from.
struct FanartChoice: Decodable, Identifiable, Hashable {
    var url: String
    var preview: String
    var likes: Int
    var language: String
    var id: String { url }
}

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
/// Picture… (a file), Pick from fanart.tv…, and Reset (back to the one Needle found).
struct ArtistPicturesMenu: View {
    private var store: Store { .shared }
    let name: String
    /// Called after a picture changed, to show it.
    let changed: () -> Void
    @State private var own: [String] = []
    @State private var picking: PictureKind?

    var body: some View {
        Menu {
            ForEach(PictureKind.allCases) { kind in
                Section(kind.title) {
                    Button("Choose Picture…") { choose(kind) }
                    Button("Pick from fanart.tv…") { picking = kind }
                        .disabled(!store.settings.onlineMedia)
                    if own.contains(kind.rawValue) {
                        Button("Reset") { reset(kind) }
                    }
                }
            }
        } label: {
            Image(systemName: "photo.on.rectangle").font(.system(size: 15, weight: .semibold))
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonBorderShape(.circle)
        .glassButton()
        .controlSize(.extraLarge)
        .help("The artist's pictures")
        .task(id: "swap-\(name)") {
            // For tests: `--swap-artist-pictures <first> <second>` sets the photo, wide picture,
            // and logo to the first file, then 10 seconds later to the second, as choosing does.
            let args = CommandLine.arguments
            guard let i = args.firstIndex(of: "--swap-artist-pictures"), args.indices.contains(i + 2) else { return }
            for file in [args[i + 1], args[i + 2]] {
                try? await Task.sleep(for: .seconds(10))
                let name = name
                for kind in PictureKind.allCases {
                    _ = await store.ask { try $0.setArtistPicture(name: name, kind: kind.rawValue, path: file) }
                }
                await readOwn()
                picturesChanged()
            }
        }
        .task(id: name) { await readOwn() }
        .sheet(item: $picking) { kind in
            FanartPicker(name: name, kind: kind) {
                Task { await readOwn() }
                picturesChanged()
            }
        }
    }

    private func readOwn() async {
        let name = name
        own = await store.load { core in
            PictureKind.allCases.filter { (try? core.ownArtistPicture(name: name, kind: $0.rawValue)) ?? nil != nil }.map(\.rawValue)
        } ?? []
    }

    private func choose(_ kind: PictureKind) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.jpeg, .png, .webP, .gif]
        panel.message = "Choose a picture for \(name)'s \(kind.title.lowercased())."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let (name, path) = (name, url.path)
        Task {
            if await store.ask({ try $0.setArtistPicture(name: name, kind: kind.rawValue, path: path) }) != nil {
                await readOwn()
                picturesChanged()
            }
        }
    }

    private func reset(_ kind: PictureKind) {
        let name = name
        Task {
            if await store.ask({ try $0.resetArtistPicture(name: name, kind: kind.rawValue) }) != nil {
                await readOwn()
                picturesChanged()
            }
        }
    }

    /// A picture changed: the picture views read their files again, and the page its pictures.
    private func picturesChanged() {
        PictureChanges.shared.count += 1
        changed()
    }
}

/// All of fanart.tv's choices for one of an artist's pictures; a click uses that one.
struct FanartPicker: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let name: String
    let kind: PictureKind
    let picked: () -> Void
    @State private var choices: [FanartChoice]?
    @State private var busy: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("\(name): \(kind.title.lowercased()) from fanart.tv").font(.needle(.headline))
            Group {
                if let choices, choices.isEmpty {
                    Text("fanart.tv has no \(kind.title.lowercased()) of \(name).")
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if let choices {
                    ScrollView {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: kind == .photo ? 130 : 220), spacing: 12)], spacing: 12) {
                            ForEach(choices) { choice in tile(choice) }
                        }
                        .padding(2)
                        if choices.count == 1 {
                            Text("This is the only one fanart.tv has of this artist. Choose Picture… to use another.")
                                .font(.needle(.callout)).foregroundStyle(.secondary)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.top, 6)
                        }
                    }
                } else {
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(height: 380)
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(22)
        .frame(width: 620)
        .task {
            let (name, kind) = (name, kind.rawValue)
            choices = JSON.decode([FanartChoice].self, await store.load { try $0.artistFanartChoicesJson(name: name, kind: kind) }) ?? []
        }
    }

    private func tile(_ choice: FanartChoice) -> some View {
        Button { pick(choice) } label: {
            AsyncImage(url: URL(string: choice.preview)) { image in
                image.resizable().scaledToFit()
            } placeholder: {
                Color.primary.opacity(0.06)
            }
            .frame(height: kind == .photo ? 130 : 90)
            .frame(maxWidth: .infinity)
            // Grey behind logos: fanart.tv has white ones and black ones.
            .background(kind == .logo ? Color(white: 0.42) : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay {
                if busy == choice.url { ProgressView().controlSize(.small) }
            }
        }
        .buttonStyle(.plain)
        .disabled(busy != nil)
        .help(choice.likes > 0 ? "\(choice.likes) likes on fanart.tv" : "Use this one")
        .accessibilityLabel("A \(kind.title.lowercased()) from fanart.tv, \(choice.likes) likes")
    }

    private func pick(_ choice: FanartChoice) {
        busy = choice.url
        let (name, kind, url) = (name, kind.rawValue, choice.url)
        Task {
            let done = await store.ask { try $0.pickFanart(name: name, kind: kind, url: url) } != nil
            busy = nil
            if done {
                picked()
                dismiss()
            }
        }
    }
}

/// Settings › Library › Artist pictures: the slow search for every artist's photo, logo, and
/// wide picture, with how far it is and Check all now.
struct ArtistPicturesRow: View {
    private var store: Store { .shared }
    @State private var progress: PictureSweepProgress?

    var body: some View {
        let s = store.settings
        SettingRow(title: "Artist pictures", detail: detail(on: s.artistPictures), icon: "person.crop.square") {
            HStack {
                if s.artistPictures {
                    Button("Check all now") { store.core.checkAllArtistPictures() }
                        .disabled(progress?.hurry == true)
                }
                SettingSwitch(key: "artist_pictures", on: s.artistPictures)
            }
        }
        .task {
            while !Task.isCancelled {
                progress = await store.load { $0.pictureSweepProgress() }
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    private func detail(on: Bool) -> String {
        guard on else { return "Photos, logos, and wide pictures for every artist, found slowly." }
        guard let progress, progress.total > 0 else { return "Finds every artist's photo, logo, and wide picture, one at a time." }
        if progress.done >= progress.total { return "All \(Int(progress.total).formatted()) artists looked for." }
        return "\(Int(progress.done).formatted()) of \(Int(progress.total).formatted()) artists looked for."
    }
}
