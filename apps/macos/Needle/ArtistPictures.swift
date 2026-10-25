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

/// How far the slow search for artists' pictures is.
struct SweepProgress: Decodable, Equatable {
    var done: Int
    var total: Int
    var allowed: Bool
    var hurry: Bool
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
        .task(id: name) { await readOwn() }
        .sheet(item: $picking) { kind in
            FanartPicker(name: name, kind: kind) {
                Task { await readOwn() }
                changed()
            }
        }
    }

    private func readOwn() async {
        let name = name
        own = await store.load { $0.artistOwnPictures(name: name) } ?? []
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
                changed()
            }
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
            Text("\(name): \(kind.title.lowercased()) from fanart.tv").font(.headline)
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
            .background(kind == .logo ? Color.black.opacity(0.6) : .clear)
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
/// wide picture, with how far it is and Check all now.
struct ArtistPicturesRow: View {
    private var store: Store { .shared }
    @State private var progress: SweepProgress?

    var body: some View {
        let s = store.settings
        SettingRow(title: "Artist pictures", detail: detail(on: s.artistPictures), icon: "person.crop.square") {
            HStack {
                if s.artistPictures {
                    Button("Check all now") { store.core.artistPicturesCheckAllNow() }
                        .disabled(progress?.hurry == true)
                }
                SettingSwitch(key: "artist_pictures", on: s.artistPictures)
            }
        }
        .task {
            while !Task.isCancelled {
                progress = JSON.decode(SweepProgress.self, await store.load { try $0.artistPicturesProgressJson() })
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    private func detail(on: Bool) -> String {
        guard on else { return "Finds every artist's photo in the background." }
        guard let progress, progress.total > 0 else { return "Finds every artist's photo in the background." }
        if progress.done >= progress.total { return "All \(progress.total.formatted()) artists checked." }
        let counted = "\(progress.done.formatted()) of \(progress.total.formatted()) artists"
        return progress.allowed ? counted : counted + " · waits while the library is read"
    }
}
