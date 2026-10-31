import UIKit
import WatchConnectivity

/// The Apple Watch app's link: the watch shows the song playing here, and its buttons (play or
/// pause, previous, next, favorite) control Needle on the iPhone.
@MainActor
final class WatchLink: NSObject, WCSessionDelegate {
    static let shared = WatchLink()
    private weak var store: Store?
    /// What was sent last: the song, playing or paused, and whether it is a favorite.
    private var sent = ""
    private var coverFor: String?
    private var cover: Data?

    func start(_ store: Store) {
        guard WCSession.isSupported(), self.store == nil else { return }
        self.store = store
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// Sends what plays to the watch, when the song, playing, or the favorite changes.
    func update(_ playback: Playback?) {
        guard let store else { return }
        let song = playback?.current
        let playing = playback?.playing == true
        let key = "\(song?.id ?? "")|\(playing)|\(store.ratingVersion)"
        guard key != sent, WCSession.default.activationState == .activated,
              WCSession.default.isPaired, WCSession.default.isWatchAppInstalled else { return }
        sent = key
        Task {
            var favorite = false
            if let id = song?.id, song?.onComputer == false {
                favorite = await store.load { $0.isFavorite(id: id) } ?? false
            }
            if coverFor != song?.artwork {
                coverFor = song?.artwork
                cover = nil
                if let path = song?.artwork { cover = await Covers.load(path, size: 160)?.jpegData(compressionQuality: 0.7) }
            }
            var context: [String: Any] = [
                "title": song?.title ?? "", "artist": song?.artist ?? "",
                "playing": playing, "favorite": favorite,
                "position": playback?.position ?? 0, "duration": song?.duration ?? 0,
                "at": Date().timeIntervalSince1970,
            ]
            if let cover { context["cover"] = cover }
            try? WCSession.default.updateApplicationContext(context)
        }
    }

    private func perform(_ action: String) {
        guard let store else { return }
        switch action {
        case "toggle": store.toggle()
        case "next": store.next()
        case "previous": store.previous()
        case "favorite":
            guard let id = store.playback?.current?.id, store.playback?.current?.onComputer == false else { return }
            Task {
                if await store.ask({ try $0.toggleFavorite(id: id) }) != nil { store.ratingsChanged() }
                update(store.playback)
            }
        default: break
        }
    }

    // MARK: WCSessionDelegate

    nonisolated func session(_ session: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {
        Task { @MainActor in
            sent = ""
            update(store?.playback)
        }
    }

    nonisolated func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
        guard let action = message["do"] as? String else { return }
        Task { @MainActor in perform(action) }
    }

    nonisolated func sessionDidBecomeInactive(_ session: WCSession) {}

    nonisolated func sessionDidDeactivate(_ session: WCSession) {
        // Another watch was chosen: talk to it.
        WCSession.default.activate()
    }
}
