import ActivityKit
import AppIntents
import Foundation

/// What the widgets, Control Center, and Siri ask the app to do. The app sets `handler` at
/// start; the system runs these intents in the app (they are audio intents), never in the
/// widgets, so the handler is always there when one runs.
enum Action: Sendable {
    case toggle, next, previous
    case playAlbum(String)
    case playPlaylist(String)
    case favourites
    case radio
}

@MainActor
enum Actions {
    static var handler: ((Action) -> Void)?
    /// Asked for before the app set `handler` (a cold start from a widget or Siri).
    private static var waiting: [Action] = []

    static func run(_ action: Action) {
        if let handler { handler(action) } else { waiting.append(action) }
    }

    /// The actions asked for before the app was ready, once.
    static func takeWaiting() -> [Action] {
        defer { waiting = [] }
        return waiting
    }
}

struct TogglePlaybackIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play or Pause"
    static let description = IntentDescription("Plays or pauses the music in Needle.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.toggle)
        return .result()
    }
}

struct NextSongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Next Song"

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.next)
        return .result()
    }
}

struct PreviousSongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Previous Song"

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.previous)
        return .result()
    }
}

struct PlayFavouritesIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play My Favourites"
    static let description = IntentDescription("Plays your favourite songs in Needle, shuffled.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.favourites)
        return .result()
    }
}

struct StartRadioIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play My Radio"
    static let description = IntentDescription("Starts radio in Needle from what you play most.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.radio)
        return .result()
    }
}

struct PlayAlbumIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Album"
    static let isDiscoverable = false

    @Parameter(title: "Album") var key: String

    init() {}
    init(key: String) { self.key = key }

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.playAlbum(key))
        return .result()
    }
}

struct PlayPlaylistIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Playlist"
    static let isDiscoverable = false

    @Parameter(title: "Playlist") var id: String

    init() {}
    init(id: String) { self.id = id }

    @MainActor
    func perform() async throws -> some IntentResult {
        Actions.run(.playPlaylist(id))
        return .result()
    }
}

/// A Live Activity: the sleep timer counting down, or songs coming in from a music server.
struct NeedleActivity: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var title: String
        var detail: String
        /// The sleep timer's end, counted down on the lock screen.
        var ends: Date?
        /// 0 to 1, or nil when the amount is not known.
        var progress: Double?
    }

    /// "sleep" or "sync".
    var kind: String
}
