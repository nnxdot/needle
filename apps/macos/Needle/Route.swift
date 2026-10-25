import Foundation

/// A page of the main window.
enum Route: Hashable {
    case home
    case songs
    case albums
    case artists
    case folders
    /// One folder on disk: its own folders, and every song in it and below it.
    case folder(String)
    case favorites
    case recent
    case history
    /// Your library in numbers.
    case numbers
    /// Every measured song by its mood.
    case moodMap
    /// A collection added to the sidebar (`Collections.more`), by its key.
    case collection(String)
    case playlist(String)
    case sound
    case importing
    /// The lyric timing editor for the song playing.
    case timing
    /// A year of listening, told back.
    case wrapped(Int)
    /// Fix my library.
    case doctor
    /// The songs of a music server a plugin connects to.
    case source(plugin: String, name: String)
    case album(album: String, artist: String, query: String)
    case artist(String)

    var title: String {
        switch self {
        case .home: "Home"
        case .songs: "Songs"
        case .albums: "Albums"
        case .artists: "Artists"
        case .folders: "Folders"
        case .folder(let path): (path as NSString).lastPathComponent
        case .favorites: "Favorites"
        case .recent: "Recently added"
        case .history: "Listening history"
        case .numbers: "Your library in numbers"
        case .moodMap: "Mood map"
        case .collection(let key): Collections.find(key)?.name ?? "Collection"
        case .playlist: "Playlist"
        case .sound: "Sound"
        case .importing: "Import"
        case .timing: "Lyric timing"
        case .wrapped(let year): "\(year) in music"
        case .doctor: "Fix my library"
        case .source(_, let name): name
        case .album(let album, _, _): album.isEmpty ? "Unknown album" : album
        case .artist(let name): name.isEmpty ? "Unknown artist" : name
        }
    }

    /// The rule behind the page, before any search is applied.
    var base: String {
        switch self {
        case .favorites: "rating >= 4"
        case .collection(let key): Collections.find(key)?.rule ?? ""
        case .recent: "recent(30d) order by added_at desc"
        case .album(_, _, let query): query
        case .artist(let name): "artist = \(Rule.quote(name)) or album_artist = \(Rule.quote(name))"
        case .folder(let path): "path starts with \(Rule.quote(path.hasSuffix("/") ? path : path + "/")) order by path"
        default: ""
        }
    }

    /// A list of songs.
    var isSongs: Bool {
        switch self {
        case .songs, .favorites, .recent, .collection, .playlist, .album, .folder, .source: true
        default: false
        }
    }

    /// Search on this page searches inside it; on others it goes to Songs.
    var searchesInside: Bool {
        switch self {
        case .songs, .favorites, .collection, .recent, .playlist, .album, .folder, .albums, .artists: true
        default: false
        }
    }

    /// Why songs played from here are playing.
    func reason(search: String, playlistName: String?) -> String {
        switch self {
        case .album(let album, _, _): "From the album \(album)"
        case .artist(let name): "From \(name)"
        case .playlist: playlistName.map { "From your playlist \($0)" } ?? "From a playlist"
        default: search.isEmpty ? "From \(title.lowercased())" : "Matches “\(search)”"
        }
    }

    /// The sidebar entry that stays lit while this page shows.
    /// An album's or artist's page: the window takes its picture's colours.
    var isPaged: Bool {
        switch self {
        case .album, .artist: true
        default: false
        }
    }

    var sidebarRoute: Route {
        switch self {
        case .album: .albums
        case .artist: .artists
        case .folder: .folders
        default: self
        }
    }
}

/// Collections that can be added to the sidebar beside Favorites.
enum Collections {
    struct Item {
        let key: String
        let name: String
        let icon: String
        let rule: String
    }

    static let more: [Item] = [
        Item(key: "rated", name: "Rated", icon: "star", rule: "rating >= 1 order by rating desc, title asc"),
        Item(key: "most-played", name: "Most played", icon: "chart.bar", rule: "play_count >= 1 order by play_count desc"),
        Item(key: "recently-played", name: "Recently played", icon: "clock.arrow.circlepath", rule: "played(30d) order by last_played desc"),
        Item(key: "never-played", name: "Never played", icon: "music.note", rule: "play_count = 0 order by added_at desc"),
    ]

    static func find(_ key: String) -> Item? { more.first { $0.key == key } }
}
