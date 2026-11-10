import SwiftUI

/// The keyboard on an iPad, after the Mac's menus (apps/macos NeedleApp.swift): the same
/// commands and keys, in the iPad's menu bar and the list that shows while ⌘ is held. A
/// window's places come from its own root (`WindowActions`).
struct NeedleCommands: Commands {
    @FocusedValue(\.windowActions) private var window
    /// What is open in that window: its player, sheets, and Settings.
    @FocusedValue(\.windowState) private var scene

    private var store: Store? { Launch.shared.store }

    var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { scene?.showSettings = true }
                .keyboardShortcut(",")
        }
        CommandGroup(after: .sidebar) {
            Button(scene?.showPlayer == true ? "Close the Player" : "Player") { togglePlayer() }
                .keyboardShortcut("p")
                .disabled(store?.playback?.current == nil)
            Button("Show Lyrics") { openPane("lyrics") }
                .keyboardShortcut("l")
                .disabled(store?.playback?.current == nil)
            Button("Show Up Next") { openPane("queue") }
                .keyboardShortcut("j")
                .disabled(store?.playback?.current == nil)
            Divider()
            Button("Search") { scene?.showPlayer = false; window?.search() }
                .keyboardShortcut("f")
                .disabled(window == nil)
        }
        CommandMenu("Controls") {
            // Space and the arrows are keys of typing too: while text is typed anywhere in
            // Needle these commands are off, so the keys type a space or move in the text.
            let typing = Typing.shared.active
            Button(store?.isPlaying == true ? "Pause" : "Play") { store?.toggle() }
                .keyboardShortcut(.space, modifiers: [])
                .disabled(store?.playback?.current == nil || typing)
            Group {
                Button("Next") { store?.next() }.keyboardShortcut(.rightArrow)
                Button("Previous") { store?.previous() }.keyboardShortcut(.leftArrow)
                Button("Forward 10 Seconds") { store?.jumpBy(10) }.keyboardShortcut(.rightArrow, modifiers: [.command, .option])
                Button("Back 10 Seconds") { store?.jumpBy(-10) }.keyboardShortcut(.leftArrow, modifiers: [.command, .option])
            }
            .disabled(typing)
            Divider()
            Button(store?.choices.shuffleStays == true ? "Shuffle" : "Shuffle Up Next") { store?.shuffleButton() }
            Button(repeatLabel) { store?.cycleRepeat() }.keyboardShortcut("r")
            Divider()
            Button((store?.playback?.current?.rating ?? 0) >= 4 ? "Remove from Favorites" : "Add to Favorites") { toggleFavorite() }
                .keyboardShortcut("d", modifiers: [.command, .shift])
                .disabled(store?.playback?.current == nil || store?.onComputer == true)
            Button("Sleep Timer…") { scene?.sheet = .sleep }
            Button("Speed and Pitch…") { scene?.sheet = .speed }
                .disabled(store?.onComputer == true)
            Button("Play On…") { scene?.sheet = .speakers }
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
                Button("Go to the Album Playing") { scene?.opening = .albumOf(song.id) }
                Button("Go to the Artist Playing") { scene?.opening = .artist(song.artist) }
                    .disabled(isPlaceholderArtist(song.artist))
            }
        }
    }

    private func go(_ title: String, _ place: Place, _ key: KeyEquivalent?) -> some View {
        Button(title) {
            scene?.showPlayer = false
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
        guard let store, let scene else { return }
        if scene.showPlayer { scene.showPlayer = false } else if store.playback?.current != nil { scene.showPlayer = true }
    }

    /// Opens the player with the lyrics or the queue beside the cover (in a narrow window, in
    /// its place).
    private func openPane(_ pane: String) {
        guard let store else { return }
        scene?.openPlayer(with: pane, store: store)
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

/// Text being typed somewhere in Needle (a search, a playlist's name, a server's address). The
/// keyboard's commands whose keys are also keys of typing (Space, and the arrows with ⌘ or ⌥⌘
/// that move in text) are off meanwhile.
@Observable @MainActor
final class Typing {
    static let shared = Typing()
    private(set) var active = false
    @ObservationIgnored private var watches: [NSObjectProtocol] = []

    private init() {
        let changes: [(Notification.Name, Bool)] = [
            (UITextField.textDidBeginEditingNotification, true), (UITextField.textDidEndEditingNotification, false),
            (UITextView.textDidBeginEditingNotification, true), (UITextView.textDidEndEditingNotification, false),
        ]
        for (name, on) in changes {
            watches.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.active = on
                    // Typing over: Esc is the top presentation's again.
                    if !on { EscapeKey.Catcher.reclaim() }
                }
            })
        }
    }
}

/// Esc from a hardware keyboard closes what shows on top (the player, Settings, a sheet, a
/// tool). iPadOS gives Esc to no menu command, so each of these holds a small invisible
/// responder that takes Esc before the system does; the one shown last has it. A field being
/// typed in keeps Esc, and gives it back when the typing ends.
struct EscapeKey: UIViewRepresentable {
    let action: () -> Void

    func makeUIView(context: Context) -> Catcher { Catcher() }

    func updateUIView(_ view: Catcher, context: Context) {
        view.action = action
    }

    final class Catcher: UIView {
        var action: () -> Void = {}

        private struct Held {
            weak var view: Catcher?
        }

        /// Shown, the last on top.
        private static var shown: [Held] = []

        override var canBecomeFirstResponder: Bool { true }

        override var keyCommands: [UIKeyCommand]? {
            // Esc, and ⌘. (an iPad keyboard's other Cancel).
            let esc = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(close))
            let period = UIKeyCommand(input: ".", modifierFlags: .command, action: #selector(close))
            for command in [esc, period] { command.wantsPriorityOverSystemBehavior = true }
            return [esc, period]
        }

        @objc private func close() { action() }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            Self.shown.removeAll { $0.view == nil || $0.view === self }
            if window != nil { Self.shown.append(Held(view: self)) }
            Self.reclaim()
        }

        /// The top one in the key window takes Esc, unless text is being typed.
        static func reclaim() {
            DispatchQueue.main.async {
                guard !Typing.shared.active,
                      let top = shown.last(where: { $0.view?.window?.isKeyWindow == true })?.view else { return }
                if !top.isFirstResponder { top.becomeFirstResponder() }
            }
        }
    }
}
