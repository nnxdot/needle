import UIKit
import WatchConnectivity

/// The Apple Watch app's link: the watch shows the song playing here, and its buttons (play or
/// pause, previous, next, favorite) control Needle on the iPhone.
@MainActor
final class WatchLink: NSObject, WCSessionDelegate {
    static let shared = WatchLink()
    private weak var store: Store?
    /// What was sent last: the song, playing or paused, the speed, and whether it is a favorite.
    private var sent = ""
    /// Where the song was when it was last sent, when, and how fast it went on: a seek is a
    /// jump away from where that says it is now.
    private var sentPosition = 0.0
    private var sentAt = Date.distantPast
    private var sentRate = 0.0
    private var coverFor: String?
    private var cover: Data?

    func start(_ store: Store) {
        guard WCSession.isSupported(), self.store == nil else { return }
        self.store = store
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// Sends what plays to the watch, when the song, playing, the speed, or the favorite
    /// changes, or the song jumped (a seek).
    func update(_ playback: Playback?, speed: Double) {
        guard let store else { return }
        let song = playback?.current
        let playing = playback?.playing == true
        let position = playback?.position ?? 0
        let key = "\(song?.id ?? "")|\(playing)|\(store.ratingVersion)|\(speed)"
        let expected = sentPosition + Date().timeIntervalSince(sentAt) * sentRate
        let jumped = abs(expected - position) > 2
        guard key != sent || jumped, WCSession.default.activationState == .activated,
              WCSession.default.isPaired, WCSession.default.isWatchAppInstalled else { return }
        sent = key
        sentPosition = position
        sentAt = Date()
        sentRate = playing ? speed : 0
        Task {
            let context = await context(playback, speed: speed)
            try? WCSession.default.updateApplicationContext(context)
        }
    }

    /// What the watch shows: the song, playing or paused, the favorite, where the song is and
    /// how fast it goes on, when that was, and the cover.
    private func context(_ playback: Playback?, speed: Double) async -> [String: Any] {
        let song = playback?.current
        let playing = playback?.playing == true
        var favorite = false
        if let store, let id = song?.id, song?.onComputer == false {
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
            "position": store?.clock.position ?? playback?.position ?? 0, "duration": song?.duration ?? 0,
            "speed": speed,
            "at": Date().timeIntervalSince1970,
        ]
        if let cover { context["cover"] = cover }
        return context
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
                sent = ""
                update(store.playback, speed: Double(store.tempo.speed))
            }
        default: break
        }
    }

    // MARK: WCSessionDelegate

    nonisolated func session(_ session: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {
        Task { @MainActor in
            sent = ""
            update(store?.playback, speed: Double(store?.tempo.speed ?? 1))
        }
    }

    nonisolated func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
        guard let action = message["do"] as? String else { return }
        Task { @MainActor in perform(action) }
    }

    /// The watch asks what plays now (it opened, or what it last heard is old): the answer
    /// comes back at once, from Needle as it is (started anew, it plays nothing).
    nonisolated func session(_ session: WCSession, didReceiveMessage message: [String: Any], replyHandler: @escaping ([String: Any]) -> Void) {
        let action = message["do"] as? String
        let reply = UncheckedReply(send: replyHandler)
        Task { @MainActor in
            if let action, action != "state" { perform(action) }
            let speed = Double(store?.tempo.speed ?? 1)
            reply.send(await context(store?.playback, speed: speed))
        }
    }

    nonisolated func sessionDidBecomeInactive(_ session: WCSession) {}

    nonisolated func sessionDidDeactivate(_ session: WCSession) {
        // Another watch was chosen: talk to it.
        WCSession.default.activate()
    }
}

/// WatchConnectivity's reply, carried to the main actor (it may be called from any thread).
private struct UncheckedReply: @unchecked Sendable {
    let send: ([String: Any]) -> Void
}
