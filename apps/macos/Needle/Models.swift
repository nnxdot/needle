import Foundation

/// The core sends rich data as JSON with the desktop's own names (`album_artist`); these read it.
enum JSON {
    static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    /// Without changing names, for data whose keys are names of their own (theme colours).
    static let plain = JSONDecoder()

    static func decode<T: Decodable>(_ type: T.Type, _ text: String?, plain: Bool = false) -> T? {
        guard let data = text?.data(using: .utf8) else { return nil }
        do {
            return try (plain ? Self.plain : decoder).decode(type, from: data)
        } catch {
            NSLog("Needle could not read \(type): \(error)")
            return nil
        }
    }

    static func text(_ object: Any) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.fragmentsAllowed]) else { return "{}" }
        return String(decoding: data, as: UTF8.self)
    }
}

/// A song with every field the library knows.
struct Track: Codable, Identifiable, Hashable {
    var id: String
    var path: String
    var title: String
    var artist: String
    var album: String
    var albumArtist: String
    var genre: String
    var year: Int
    var trackNumber: Int
    var disc: Int
    var duration: Double
    var sampleRate: Int
    var bitDepth: Int
    var channels: Int
    var format: String
    var bitrate: Int
    var bpm: Double?
    var rating: Int
    var addedAt: Int
    var fileSize: Int
    var artwork: String?
    var replayGain: Double?
    var albumReplayGain: Double?
    var musicbrainzId: String?
    var playCount: Int
    var lastPlayed: Int?
    var missing: Bool
    var cue: Cue?

    struct Cue: Codable, Hashable {
        var audio: String
        var start: Double
        var end: Double?
    }

    var displayArtist: String { artist.isEmpty ? "Unknown artist" : artist }
    var displayAlbum: String { album.isEmpty ? "Unknown album" : album }
    var displayAlbumArtist: String { albumArtist.isEmpty ? artist : albumArtist }
    var streamed: Bool { path.hasPrefix("source://") }
    var favorite: Bool { rating >= 4 }
    /// The file on disk: the `.cue` sheet for a song of one.
    var filePath: String {
        guard cue != nil, let hash = path.lastIndex(of: "#") else { return path }
        return String(path[..<hash])
    }
    /// The plugin a song from a music server comes from.
    var sourcePlugin: String? {
        guard streamed else { return nil }
        return path.dropFirst("source://".count).split(separator: "/").first.map(String.init)
    }

    /// The format and resolution, as the desktop shows it: "FLAC 24/96", "MP3 320".
    var quality: String {
        let name = format.uppercased()
        let lossless = ["FLAC", "ALAC", "WAV", "AIFF", "APE", "WV", "DSF", "DFF"].contains(name)
        if lossless, bitDepth > 0, sampleRate > 0 {
            let khz = Double(sampleRate) / 1000
            let rate = khz.rounded() == khz ? String(Int(khz)) : String(format: "%.1f", khz)
            return "\(name) \(bitDepth)/\(rate)"
        }
        if bitrate > 0 { return "\(name) \(bitrate)" }
        return name
    }

    /// The album page for this song, grouped as the library groups albums.
    var albumRoute: Route {
        if albumArtist.isEmpty {
            return .album(album: album, artist: artist,
                          query: "album_artist = \"\" and artist = \(Rule.quote(artist)) and album = \(Rule.quote(album))")
        }
        return .album(album: album, artist: albumArtist,
                      query: "album_artist = \(Rule.quote(albumArtist)) and album = \(Rule.quote(album))")
    }
}

struct TracksPage: Decodable {
    var tracks: [Track]
    var total: Int
}

struct AlbumSummary: Codable, Identifiable, Hashable {
    var key: String
    var query: String
    var album: String
    var artist: String
    var year: Int
    var tracks: Int
    var duration: Double
    var artwork: String?
    var formats: String
    var addedAt: Int
    var playCount: Int
    var id: String { key }
    var title: String { album.isEmpty ? "Unknown album" : album }
    var route: Route { .album(album: album, artist: artist, query: query) }
}

struct ArtistSummary: Codable, Identifiable, Hashable {
    var name: String
    var albums: Int
    var tracks: Int
    var artwork: String?
    /// The letter the artist files under in the A–Z bar ("A"–"Z", "#", or another script's).
    var letter: String
    var id: String { name }
    var title: String { name.isEmpty ? "Unknown artist" : name }
}

struct HomeData: Decodable {
    var recent: [AlbumSummary]
    var added: [AlbumSummary]
    var favorites: [AlbumSummary]
    var mostPlayed: [AlbumSummary]
    var artists: [ArtistSummary]
}

/// A playlist as the library keeps it.
struct PlaylistModel: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var query: String?
    var trackIds: [String]
    var updatedAt: Int
    var description: String
    var cover: String?
    var rules: RuleSet?

    var smart: Bool { query != nil }

    static func new() -> PlaylistModel {
        PlaylistModel(id: "", name: "", query: nil, trackIds: [], updatedAt: 0, description: "", cover: nil, rules: nil)
    }
}

struct RuleSet: Codable, Hashable {
    var any: Bool
    var rules: [Rule]
    var order: String
    var limit: UInt32?
}

struct Rule: Codable, Hashable, Identifiable {
    var field: String
    var op: String
    var value: String
    var id = UUID()

    enum CodingKeys: String, CodingKey { case field, op, value }

    static func quote(_ text: String) -> String {
        "\"" + text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }
}

struct RuleKinds: Decodable {
    var kinds: [Kind]
    var orders: [String]
    struct Kind: Decodable, Hashable {
        var label: String
        var ops: [String]
        var value: String
        var placeholder: String
    }
}

/// Needle's settings, as the desktop keeps them. Only what the app reads is here; changes are
/// sent by name (see `Store.set`), so the rest stays as it is.
/// The core keeps this as a pair: [grain, font].
struct ExtrasBefore: Decodable, Equatable {
    var grain: Float
    var font: String

    init(from decoder: Decoder) throws {
        var pair = try decoder.unkeyedContainer()
        grain = try pair.decode(Float.self)
        font = try pair.decode(String.self)
    }
}

struct NeedleSettings: Decodable, Equatable {
    var volume: Float = 0.65
    var outputDevice: String?
    var replayGain = false
    var albumGain = false
    var autoplayQuery = ""
    var theme = "dark"
    var showInspector = true
    var lastfmEnabled = false
    var listenbrainzEnabled = false
    var scrobbleCorrections = true
    var layout = Layout()
    var onlineMedia = false
    var coverColors = true
    var immersiveCover = true
    var serverMark = true
    var playerLyrics = false
    var animatedCovers = true
    var soundAnalysis = true
    var checkUpdates = true
    var lastUpdateCheck = 0
    var reduceMotion = false
    var musicColors = true
    var windowMaterial = "glass"
    var glassAmount: Float = 0.6
    var grain: Float = 0
    /// The grain and the font from before a theme with its own was chosen, to put back.
    var themeExtrasBefore: ExtrasBefore?
    var discordPresence = true
    var discordCovers = true
    var discordTitle = "song"
    var discordTop = "artist"
    var discordMiddle = "song"
    var discordBottom = "album"
    var discordLogo = true
    var discordPaused = true
    var discordIdleMinutes = 10
    var displayFont = "system"
    var textFont = "system"
    var playerButtons = PlayerButtons.core
    var artistOrder = "name"
    var albumArtistsOnly = false
    var artistPictures = true
    var accentColor = ""
    var ambient = false
    var crossfade: Float = 0
    var smartMix = true
    /// The big player's look: "cover", "record", "lyrics", or "visualizer".
    var bigLook = "cover"
    var crashReports = true
    var welcomed = false
    var whatsNewSeen = ""
    var remote = false
    var tray = false
    /// The sleep timer puts the Mac to sleep when it ends.
    var sleepComputer = false

    struct Layout: Decodable, Equatable {
        var sidebarWidth: Float = 212
        var inspectorWidth: Float = 284
        var rowHeight: Float = 52
        var sidebarHidden = false
        var compactSidebar = false
        var searchHidden = false
        var playerOnTop = false
        var collections: [String] = []
    }
}

/// Recordings MusicBrainz or AcoustID found for a song.
struct RecordingMatch: Decodable, Identifiable, Hashable {
    var id: String
    var title: String
    var artist: String
    var album: String
    var score: Int
    var releaseId: String?
}

/// What several songs have in common, field by field.
struct CommonTags: Decodable {
    var count: Int
    var title: Common<String>
    var artist: Common<String>
    var album: Common<String>
    var albumArtist: Common<String>
    var genre: Common<String>
    var year: Common<Int>
    var trackNumber: Common<Int>

    struct Common<T: Decodable>: Decodable {
        var state: String
        var value: T?
    }
}

struct Speaker: Codable, Hashable, Identifiable {
    var kind: String
    var name: String
    var address: String
    var id: String { kind + address }
    var label: String {
        switch kind {
        case "Dlna": "DLNA"
        case "AirPlay": "AirPlay"
        default: kind
        }
    }
}

struct ServiceInfo: Decodable {
    var status: Status
    var summary: Summary?

    struct Status: Decodable {
        var lastfm: State
        var lastfmApp: State
        var listenbrainz: State
        var acoustid: State
        var storeError: String?
    }
    struct State: Decodable {
        var configured: Bool
        var source: String?
        var user: String?
        var rejected: String?
    }
    struct Summary: Decodable {
        var lastfm: Queue
        var listenbrainz: Queue
    }
    struct Queue: Decodable {
        var pending: Int
        var sent: Int
        var failed: Int
        var lastError: String?
        var signInRequired: String?
    }
}

struct Detected: Decodable, Identifiable {
    var kind: String
    var path: String
    var label: String
    var detail: String
    var id: String { kind + path }
}

/// A theme file, as Settings › Appearance lists it.
struct ThemeFile: Decodable, Identifiable, Hashable {
    var id: String
    var name: String
    var base: String
    var author: String
    var readOnly: Bool
    var musicColors: Bool
    var colors: [String: String]
    var grain: Double?
    var font: String?
    var path: String

    enum CodingKeys: String, CodingKey {
        case id, name, base, author, colors, grain, font, path
        case readOnly = "read_only"
        case musicColors = "music_colors"
    }
}

struct PluginUpdate: Decodable {
    var id: String
    var update: String
}

extension Song {
    /// The library's whole track for this song (album artist, file, source), for its menu: the
    /// song's own shape lacks them, and Go to Album, Show in Finder, and Keep need them.
    @MainActor var fullTrack: Track {
        let version = Store.shared.libraryVersion
        if let kept = Song.whole[id], kept.version == version { return kept.track }
        guard let found = JSON.decode(Track.self, (try? Store.shared.core.trackJson(id: id)) ?? nil) else { return asTrack }
        // Kept until the library changes (a rating, a tag): then read again.
        if Song.whole.count > 500 { Song.whole.removeAll() }
        Song.whole[id] = (version, found)
        return found
    }

    @MainActor private static var whole: [String: (version: Int, track: Track)] = [:]

    /// A song of the phones' shape, as some of the core's answers give it.
    var asTrack: Track {
        Track(id: id, path: "", title: title, artist: artist, album: album, albumArtist: "", genre: "", year: 0,
              trackNumber: Int(trackNumber), disc: 0, duration: duration, sampleRate: 0, bitDepth: 0, channels: 0,
              format: format, bitrate: 0, bpm: nil, rating: Int(rating), addedAt: 0, fileSize: 0, artwork: artwork,
              replayGain: nil, albumReplayGain: nil, musicbrainzId: nil, playCount: 0, lastPlayed: nil, missing: false, cue: nil)
    }
}

/// "3:07", or "1:02:03".
func clock(_ seconds: Double) -> String {
    let s = Int(max(0, seconds.isFinite ? seconds : 0))
    return s >= 3600 ? String(format: "%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60) : String(format: "%d:%02d", s / 60, s % 60)
}

/// "42 min", or "3 h 5 min".
func longTime(_ seconds: Double) -> String {
    let minutes = Int((seconds / 60).rounded())
    return minutes >= 60 ? "\(minutes / 60) h \(minutes % 60) min" : "\(minutes) min"
}

/// "1 song", "12 songs".
func count(_ n: Int, _ noun: String) -> String {
    "\(n.formatted()) \(noun)\(n == 1 ? "" : "s")"
}

/// "12 Mar 2026".
func day(_ seconds: Int?) -> String {
    guard let seconds, seconds > 0 else { return "" }
    return Date(timeIntervalSince1970: TimeInterval(seconds)).formatted(.dateTime.day().month(.abbreviated).year())
}

extension Error {
    /// What went wrong, in words: the message of an error from Needle's core (whose own
    /// description is its Swift name), or the system's description.
    var message: String {
        if let needle = self as? NeedleError, case .Failed(let text) = needle { return text }
        return localizedDescription
    }
}
