import SwiftUI

/// The keyboard on an iPad, after the Mac's menus (apps/macos NeedleApp.swift): the same
/// commands and keys, in the iPad's menu bar and the list that shows while ⌘ is held. A
/// window's places come from its own root (`WindowActions`).
struct NeedleCommands: Commands {
    @FocusedValue(\.windowActions) private var window

    private var store: Store? { Launch.shared.store }

    var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { store?.showSettings = true }
                .keyboardShortcut(",")
        }
        CommandGroup(after: .sidebar) {
            Button(store?.showPlayer == true ? "Close the Player" : "Player") { togglePlayer() }
                .keyboardShortcut("p")
                .disabled(store?.playback?.current == nil)
            Button("Show Lyrics") { openPane("lyrics") }
                .keyboardShortcut("l")
                .disabled(store?.playback?.current == nil)
            Button("Show Up Next") { openPane("queue") }
                .keyboardShortcut("j")
                .disabled(store?.playback?.current == nil)
            Divider()
            Button("Search") { store?.showPlayer = false; window?.search() }
                .keyboardShortcut("f")
                .disabled(window == nil)
        }
        CommandMenu("Controls") {
            // Space plays and pauses while no text is being typed: a field being typed in
            // takes its own keys first.
            Button(store?.isPlaying == true ? "Pause" : "Play") { store?.toggle() }
                .keyboardShortcut(.space, modifiers: [])
                .disabled(store?.playback?.current == nil)
            Button("Next") { store?.next() }.keyboardShortcut(.rightArrow)
            Button("Previous") { store?.previous() }.keyboardShortcut(.leftArrow)
            Button("Forward 10 Seconds") { store?.jumpBy(10) }.keyboardShortcut(.rightArrow, modifiers: [.command, .option])
            Button("Back 10 Seconds") { store?.jumpBy(-10) }.keyboardShortcut(.leftArrow, modifiers: [.command, .option])
            Divider()
            Button(store?.choices.shuffleStays == true ? "Shuffle" : "Shuffle Up Next") { store?.shuffleButton() }
            Button(repeatLabel) { store?.cycleRepeat() }.keyboardShortcut("r")
            Divider()
            Button((store?.playback?.current?.rating ?? 0) >= 4 ? "Remove from Favorites" : "Add to Favorites") { toggleFavorite() }
                .keyboardShortcut("d", modifiers: [.command, .shift])
                .disabled(store?.playback?.current == nil || store?.onComputer == true)
            Button("Sleep Timer…") { store?.sheet = .sleep }
            Button("Speed and Pitch…") { store?.sheet = .speed }
                .disabled(store?.onComputer == true)
            Button("Play On…") { store?.sheet = .speakers }
        }
        CommandMenu("Go") {
            Button("Back") { window?.back() }.keyboardShortcut("[")
            Divider()
            go("Home", .home, "1")
            go("Songs", .part(.songs), "2")
            go("Albums", .part(.albums), "3")
            go("Artists", .part(.artists), "4")
            go("Favorites", .part(.favorites), "5")
            go("Recently Added", .part(.recent), "6")
            go("Listening History", .part(.history), "7")
            go("Playlists", .part(.playlists), nil)
            go("Folders", .part(.folders), nil)
            go("Radio", .radio, nil)
            if let song = store?.playback?.current, !song.onComputer {
                Divider()
                Button("Go to the Album Playing") { store?.opening = .albumOf(song.id) }
                Button("Go to the Artist Playing") { store?.opening = .artist(song.artist) }
                    .disabled(isPlaceholderArtist(song.artist))
            }
        }
    }

    private func go(_ title: String, _ place: Place, _ key: KeyEquivalent?) -> some View {
        Button(title) {
            store?.showPlayer = false
            window?.go(place)
        }
        .keyboardShortcut(key.map { KeyboardShortcut($0) })
        .disabled(window == nil)
    }

    private var repeatLabel: String {
        switch store?.playback?.repeat {
        case .all: "Repeat: All"
        case .one: "Repeat: One"
        default: "Repeat: Off"
        }
    }

    private func togglePlayer() {
        guard let store else { return }
        if store.showPlayer { store.showPlayer = false } else if store.playback?.current != nil { store.showPlayer = true }
    }

    /// Opens the player with the lyrics or the queue beside the cover (in a narrow window, in
    /// its place).
    private func openPane(_ pane: String) {
        store?.openPlayer(with: pane)
    }

    private func toggleFavorite() {
        guard let store, let id = store.playback?.current?.id else { return }
        Task {
            if let now = await store.ask({ try $0.toggleFavorite(id: id) }) {
                store.ratingsChanged()
                store.message = now ? "Added to Favorites" : "Removed from Favorites"
            }
        }
    }
}
