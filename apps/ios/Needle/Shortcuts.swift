import AppIntents

/// What Siri, Spotlight, and the Shortcuts app offer for Needle.
struct NeedleShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: PlayFavouritesIntent(), phrases: ["Play my favourites in \(.applicationName)", "Play my favorites in \(.applicationName)"],
                    shortTitle: "Play Favorites", systemImageName: "star.fill")
        AppShortcut(intent: StartRadioIntent(), phrases: ["Play my radio in \(.applicationName)", "Start radio in \(.applicationName)"],
                    shortTitle: "Play My Radio", systemImageName: "dot.radiowaves.left.and.right")
        AppShortcut(intent: TogglePlaybackIntent(), phrases: ["Pause \(.applicationName)", "Play \(.applicationName)"],
                    shortTitle: "Play or Pause", systemImageName: "playpause.fill")
        AppShortcut(intent: NextSongIntent(), phrases: ["Next song in \(.applicationName)", "Skip in \(.applicationName)"],
                    shortTitle: "Next Song", systemImageName: "forward.fill")
    }
}
