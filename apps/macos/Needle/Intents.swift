import AppIntents
import Foundation

// What the Shortcuts app, Siri, and Spotlight can ask Needle to do: the iPhone's actions, and a
// playlist to choose and the song playing to favorite. They run in the app (audio intents).

struct TogglePlaybackIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play or Pause"
    static let description = IntentDescription("Plays or pauses the music in Needle.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Store.shared.toggle()
        return .result()
    }
}

struct NextSongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Next Song"
    static let description = IntentDescription("Skips to the next song in Needle.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Store.shared.next()
        return .result()
    }
}

struct PreviousSongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Previous Song"
    static let description = IntentDescription("Goes back to the previous song in Needle.")

    @MainActor
    func perform() async throws -> some IntentResult {
        Store.shared.previous()
        return .result()
    }
}

struct PlayFavouritesIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play My Favorites"
    static let description = IntentDescription("Plays your favorite songs in Needle, shuffled.")

    @MainActor
    func perform() async throws -> some IntentResult {
        let found = await Store.shared.load { try $0.favorites().count } ?? 0
        guard found > 0 else { throw NeedleIntentError.say("No favorites yet") }
        try await play(rule: Route.favorites.base, playlist: nil, reason: "From your favorites", shuffle: true)
        return .result()
    }
}

struct StartRadioIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play My Radio"
    static let description = IntentDescription("Starts radio in Needle from what you play most.")

    @MainActor
    func perform() async throws -> some IntentResult {
        let core = Store.shared.core
        try await Task.detached {
            // From the song played most, or any song when nothing has played yet; the first
            // that can start one (a song whose file is gone cannot).
            let top = try core.stats(days: 30).topSongs.compactMap(\.songId)
            let any = try core.songs().randomElement()?.id
            for id in top + [any].compactMap({ $0 }) where (try? core.radioFrom(id: id)) != nil { return }
            throw NeedleIntentError.say("Add music first")
        }.value
        return .result()
    }
}

struct FavoriteSongIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Favorite This Song"
    static let description = IntentDescription("Adds the song playing in Needle to your favorites, or takes it out.")

    @MainActor
    func perform() async throws -> some IntentResult {
        guard let current = Store.shared.current else { throw NeedleIntentError.say("Nothing is playing") }
        Store.shared.toggleFavorite([current])
        return .result()
    }
}

struct PlayPlaylistIntent: AudioPlaybackIntent {
    static let title: LocalizedStringResource = "Play Playlist"
    static let description = IntentDescription("Plays one of your playlists in Needle.")

    @Parameter(title: "Playlist") var playlist: PlaylistEntity

    init() {}
    init(playlist: PlaylistEntity) { self.playlist = playlist }

    static var parameterSummary: some ParameterSummary {
        Summary("Play \(\.$playlist)")
    }

    @MainActor
    func perform() async throws -> some IntentResult {
        let id = playlist.id
        // Gone meanwhile: never an empty rule, which plays the whole library.
        guard let model = Store.shared.playlistModel(id) else { throw NeedleIntentError.say("That playlist is gone") }
        let rule = model.query ?? ""
        try await play(rule: rule, playlist: rule.isEmpty ? id : nil, reason: "From your playlist \(model.name)", shuffle: false)
        return .result()
    }
}

/// Plays what a rule (or a playlist of picked songs) holds, as a page's Play does.
@MainActor
private func play(rule: String, playlist: String?, reason: String, shuffle: Bool) async throws {
    let core = Store.shared.core
    let n = try await Task.detached {
        try core.playView(query: rule, playlist: playlist, filter: "", start: 0, shuffle: shuffle, reason: reason)
    }.value
    if n == 0 { throw NeedleIntentError.say("Nothing here can be played") }
}

/// What went wrong, in Needle's words, for Shortcuts and Siri to say.
enum NeedleIntentError: Error, CustomLocalizedStringResourceConvertible {
    case say(String)

    var localizedStringResource: LocalizedStringResource {
        switch self {
        case .say(let text): "\(text)"
        }
    }
}

/// A playlist, for Play Playlist.
struct PlaylistEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Playlist"
    static let defaultQuery = PlaylistQuery()

    let id: String
    let name: String

    var displayRepresentation: DisplayRepresentation { DisplayRepresentation(title: "\(name)") }
}

struct PlaylistQuery: EntityStringQuery {
    private func all() async -> [PlaylistEntity] {
        let lists = await Store.shared.load { try $0.playlists() } ?? []
        return lists.map { PlaylistEntity(id: $0.id, name: $0.name) }
    }

    func entities(for identifiers: [String]) async throws -> [PlaylistEntity] {
        await all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [PlaylistEntity] {
        await all().filter { $0.name.localizedCaseInsensitiveContains(string) }
    }

    func suggestedEntities() async throws -> [PlaylistEntity] {
        await all()
    }
}

/// What Siri, Spotlight, and the Shortcuts app offer for Needle without setting up anything.
struct NeedleShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: PlayFavouritesIntent(), phrases: ["Play my favorites in \(.applicationName)", "Play my favourites in \(.applicationName)"],
                    shortTitle: "Play Favorites", systemImageName: "heart.fill")
        AppShortcut(intent: StartRadioIntent(), phrases: ["Play my radio in \(.applicationName)", "Start radio in \(.applicationName)"],
                    shortTitle: "Play My Radio", systemImageName: "dot.radiowaves.left.and.right")
        AppShortcut(intent: TogglePlaybackIntent(), phrases: ["Pause \(.applicationName)", "Play \(.applicationName)"],
                    shortTitle: "Play or Pause", systemImageName: "playpause.fill")
        AppShortcut(intent: NextSongIntent(), phrases: ["Next song in \(.applicationName)", "Skip in \(.applicationName)"],
                    shortTitle: "Next Song", systemImageName: "forward.fill")
        AppShortcut(intent: PreviousSongIntent(), phrases: ["Previous song in \(.applicationName)"],
                    shortTitle: "Previous Song", systemImageName: "backward.fill")
        AppShortcut(intent: FavoriteSongIntent(), phrases: ["Favorite this song in \(.applicationName)", "Love this song in \(.applicationName)"],
                    shortTitle: "Favorite This Song", systemImageName: "heart")
        AppShortcut(intent: PlayPlaylistIntent(), phrases: ["Play \(\.$playlist) in \(.applicationName)", "Play a playlist in \(.applicationName)"],
                    shortTitle: "Play Playlist", systemImageName: "music.note.list")
    }
}
