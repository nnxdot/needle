import Foundation

/// What the widgets show, written by the app into the folder it shares with them.
struct Snapshot: Codable {
    struct Item: Codable, Hashable, Identifiable {
        var id: String
        var title: String
        var detail: String
        /// A small copy of its cover, in the shared folder.
        var cover: String?
    }

    var title: String?
    var artist: String?
    var album: String?
    var cover: String?
    var playing = false
    var recent: [Item] = []
    var playlists: [Item] = []
    /// Seconds listened in the last seven days.
    var weekSeconds: Double = 0
    var weekSongs = 0
    var topArtist: String?

    static let group = "group.fyi.nnx.needle"

    static var folder: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
    }

    static func load() -> Snapshot {
        guard let url = folder?.appendingPathComponent("widgets.json"),
              let data = try? Data(contentsOf: url),
              let snapshot = try? JSONDecoder().decode(Snapshot.self, from: data) else { return Snapshot() }
        return snapshot
    }

    func save() {
        guard let url = Snapshot.folder?.appendingPathComponent("widgets.json"),
              let data = try? JSONEncoder().encode(self) else { return }
        try? data.write(to: url, options: .atomic)
    }

    /// The cover file in the shared folder, when there is one.
    static func coverURL(_ name: String?) -> URL? {
        guard let name, let folder else { return nil }
        let url = folder.appendingPathComponent(name)
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }
}

/// Links into the app, from widgets and from Siri: `needle://album/<key>` and so on.
enum NeedleLink {
    static func album(_ key: String) -> URL { url("album", key) }
    static func playlist(_ id: String) -> URL { url("playlist", id) }
    static let player = URL(string: "needle://player")!
    static let stats = URL(string: "needle://stats")!

    private static func url(_ kind: String, _ value: String) -> URL {
        var parts = URLComponents()
        parts.scheme = "needle"
        parts.host = kind
        parts.queryItems = [URLQueryItem(name: "id", value: value)]
        return parts.url!
    }
}
