import SwiftUI

/// Opens Needle's library at start. When it cannot be opened (a full iPhone, a damaged file),
/// the app shows what happened and a way out, instead of closing at every start.
@MainActor @Observable
final class Launch {
    static let shared = Launch()

    /// The app's store, once the library is open.
    private(set) var store: Store?
    /// Why the library could not be opened.
    private(set) var problem: String?
    /// The name the old library was given when it was moved aside.
    private(set) var movedTo: String?

    private init() { open() }

    /// Opens the library (again, after a fix).
    func open() {
        do {
            let opened = try Store()
            Store.shared = opened
            store = opened
            problem = nil
        } catch {
            problem = Store.words(error)
        }
    }

    /// Moves the library aside, under a new name beside it (kept, never deleted), and starts
    /// with an empty one. The music files are not touched.
    func startEmpty() {
        let files = FileManager.default
        let folder = Store.dataFolderURL
        let format = DateFormatter()
        format.locale = Locale(identifier: "en_US_POSIX")
        format.dateFormat = "yyyy-MM-dd-HHmmss"
        let base = "library-old-\(format.string(from: Date())).db"
        var moved = false
        for suffix in ["", "-wal", "-shm"] {
            let from = folder.appendingPathComponent("library.db" + suffix)
            guard files.fileExists(atPath: from.path) else { continue }
            if (try? files.moveItem(at: from, to: folder.appendingPathComponent(base + suffix))) != nil { moved = moved || suffix.isEmpty }
        }
        if moved { movedTo = base }
        open()
    }
}

/// What shows when the library cannot be opened: why, and a way to start again.
struct RecoveryView: View {
    let launch: Launch
    @State private var asking = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 40))
                        .foregroundStyle(ThemeColors.amber)
                    Text("Needle can't open your library")
                        .font(.title.bold())
                    Text("Your music files are safe. Only Needle's list of them, your plays, stars, and playlists could not be read. This can happen when the iPhone is full, or when the library file is damaged.")
                    if let problem = launch.problem {
                        Text(problem)
                            .font(.footnote.monospaced())
                            .foregroundStyle(.secondary)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(.quaternary, in: .soft(Corner.small))
                            .textSelection(.enabled)
                    }
                    if let moved = launch.movedTo {
                        Text("The old library was kept as \(moved), in Needle's own folder. Needle still can't start an empty one: make room on the iPhone, then try again.")
                            .foregroundStyle(.secondary)
                    } else {
                        Text("If the iPhone is full, make room in Settings › General › iPhone Storage, then try again.")
                            .foregroundStyle(.secondary)
                    }
                    Button { launch.open() } label: {
                        Text("Try again").font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    .tint(ThemeColors.amber)
                    .foregroundStyle(Color(red: 0.11, green: 0.08, blue: 0.03))
                    Button { asking = true } label: {
                        Text("Start with an empty library").font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glass)
                }
                .padding(24)
                .frame(maxWidth: 560, alignment: .leading)
                .frame(maxWidth: .infinity)
            }
            .confirmationDialog("Start with an empty library?", isPresented: $asking, titleVisibility: .visible) {
                Button("Start Empty") { launch.startEmpty() }
            } message: {
                Text("The old library is moved aside and kept, not deleted. Needle then reads your music again. Plays, stars, and playlists start empty.")
            }
        }
        .preferredColorScheme(.dark)
    }
}
