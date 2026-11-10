import SwiftUI

/// What is open in one window. An iPad can show two windows of Needle (and CarPlay needs more
/// than one scene), so each window has its own player, sheets, Settings, popovers, pages, and
/// search; what plays (the song, the queue, the place in it) and the settings are the `Store`'s,
/// the same in every window.
@Observable @MainActor
final class WindowState {
    var showPlayer = false
    /// Settings as a large sheet of two columns (an iPad's wide window; an iPhone opens the
    /// page in its tab).
    var showSettings = false
    /// The new playlist's sheet, from the + of the sidebar's Playlists.
    var newPlaylist = false
    /// The player was asked to show "lyrics" or "queue" (the floating bar, the keyboard): it
    /// shows it, then clears this.
    var playerPane: String?
    /// The welcome guide, asked for again (Settings › About).
    var welcomeAgain = false
    /// A page to open, from a widget, a link, or a menu's Go to.
    var opening: Route?
    /// A sheet over everything: adding to a playlist, tags, identifying, lyrics timing, and the
    /// player's tools (popovers in a wide window).
    var sheet: Store.Sheet?
    /// Each place's page on top is in its picture's colours, dark or light (nil: the theme's).
    /// The sidebar beside it takes the same.
    var pageDark: [Place: Bool] = [:]

    /// Opens the player with the lyrics or the queue showing (beside the cover in a wide
    /// window, in its place in a narrow one).
    func openPlayer(with pane: String, store: Store) {
        guard store.playback?.current != nil else { return }
        playerPane = pane
        showPlayer = true
    }
}

extension Store {
    /// The window in front (the one last made key, or the only one), for what the app opens by
    /// itself: the player when a song starts, a link, What's new. Waits for a window when there
    /// is none yet.
    func inFront(_ act: @escaping @MainActor (WindowState) -> Void) {
        if let front = windows.front { act(front) } else { windows.waiting.append(act) }
    }
}

/// The windows open, and which is in front.
@MainActor
final class Windows {
    private var all: [Weak] = []
    private(set) weak var front: WindowState?
    /// What waits for the first window.
    fileprivate var waiting: [@MainActor (WindowState) -> Void] = []

    private struct Weak {
        weak var state: WindowState?
    }

    /// A window was made, or came to the front.
    func came(_ state: WindowState) {
        all.removeAll { $0.state == nil || $0.state === state }
        all.append(Weak(state: state))
        front = state
        let acts = waiting
        waiting = []
        acts.forEach { $0(state) }
    }

    /// A window was closed: the one made before it is in front.
    func went(_ state: WindowState) {
        all.removeAll { $0.state == nil || $0.state === state }
        if front === state { front = all.last?.state }
    }
}

extension FocusedValues {
    /// The window the keyboard's commands act in.
    @Entry var windowState: WindowState?
}

/// Finds the UIKit window a view is in, and tells when it becomes the key window (the one the
/// keyboard types into).
struct WindowReader: UIViewRepresentable {
    let found: (UIWindow) -> Void
    let becameKey: () -> Void

    func makeUIView(context: Context) -> Probe {
        Probe(found: found, becameKey: becameKey)
    }

    func updateUIView(_ view: Probe, context: Context) {
        view.found = found
        view.becameKey = becameKey
    }

    final class Probe: UIView {
        var found: (UIWindow) -> Void
        var becameKey: () -> Void
        private var watching: NSObjectProtocol?

        init(found: @escaping (UIWindow) -> Void, becameKey: @escaping () -> Void) {
            self.found = found
            self.becameKey = becameKey
            super.init(frame: .zero)
            isUserInteractionEnabled = false
            isHidden = true
        }

        required init?(coder: NSCoder) { fatalError() }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            if let watching { NotificationCenter.default.removeObserver(watching) }
            watching = nil
            guard let window else { return }
            found(window)
            if window.isKeyWindow { becameKey() }
            watching = NotificationCenter.default.addObserver(forName: UIWindow.didBecomeKeyNotification, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.becameKey() }
            }
        }
    }
}
