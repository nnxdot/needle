import SwiftUI
import WatchConnectivity
import WatchKit

/// Needle on the Apple Watch: the song playing on the iPhone, and its controls.
@main
struct NeedleWatchApp: App {
    @State private var phone = PhoneLink()

    var body: some Scene {
        WindowGroup {
            // The bar at the top (the Favorite star) needs a navigation stack on the watch.
            NavigationStack { NowPlayingView() }.environment(phone)
        }
    }
}

/// What the iPhone says plays, and the way to send it a button press.
@MainActor @Observable
final class PhoneLink: NSObject, WCSessionDelegate {
    struct NowPlaying: Sendable {
        var title = ""
        var artist = ""
        var playing = false
        var favorite = false
        var position = 0.0
        var duration = 0.0
        /// Seconds of song a second while it plays (Speed and pitch on the iPhone).
        var speed = 1.0
        /// When `position` was true.
        var at = Date()
        var cover: Data?

        /// Where the song is at `date`, by the iPhone's last word.
        func played(at date: Date) -> Double {
            playing ? position + date.timeIntervalSince(at) * speed : position
        }

        /// What the iPhone last said is old: it said a song plays, and that song would have
        /// ended a while ago (Needle on the iPhone stopped, and said nothing more).
        func stale(at date: Date) -> Bool {
            playing && duration > 0 && played(at: date) > duration + 15
        }
    }

    private(set) var now = NowPlaying()
    private(set) var cover: UIImage?
    /// The iPhone answers (Needle runs there).
    private(set) var reachable = false

    override init() {
        super.init()
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
        #if DEBUG
        // Test builds: -send next|previous|toggle|favorite presses that button after a moment.
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "-send"), args.indices.contains(i + 1) {
            let action = args[i + 1]
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(5))
                self?.send(action)
            }
        }
        #endif
    }

    /// A button pressed here: Needle on the iPhone does it, and says what plays then.
    func send(_ action: String) {
        if action == "toggle" {
            // Paused here at the place it has reached, not where it was last said to be.
            let at = Date()
            now.position = now.played(at: at)
            now.at = at
            now.playing.toggle()
        }
        if action == "favorite" { now.favorite.toggle() }
        ask(action)
    }

    /// Asks the iPhone what plays now (on opening, and when what it said is old), doing
    /// `action` first when one is given.
    func ask(_ action: String = "state") {
        guard WCSession.default.activationState == .activated, WCSession.default.isReachable else { return }
        WCSession.default.sendMessage(["do": action], replyHandler: { context in
            let next = Self.read(context)
            Task { @MainActor in self.apply(next) }
        }, errorHandler: nil)
    }

    private func apply(_ next: NowPlaying) {
        if next.cover != now.cover { cover = next.cover.flatMap(UIImage.init(data:)) }
        now = next
    }

    nonisolated private static func read(_ context: [String: Any]) -> NowPlaying {
        NowPlaying(
            title: context["title"] as? String ?? "",
            artist: context["artist"] as? String ?? "",
            playing: context["playing"] as? Bool ?? false,
            favorite: context["favorite"] as? Bool ?? false,
            position: context["position"] as? Double ?? 0,
            duration: context["duration"] as? Double ?? 0,
            speed: context["speed"] as? Double ?? 1,
            at: Date(timeIntervalSince1970: context["at"] as? Double ?? Date().timeIntervalSince1970),
            cover: context["cover"] as? Data)
    }

    // MARK: WCSessionDelegate

    nonisolated func session(_ session: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {
        let next = Self.read(session.receivedApplicationContext)
        let reachable = session.isReachable
        Task { @MainActor in
            apply(next)
            self.reachable = reachable
            ask()
        }
    }

    nonisolated func session(_ session: WCSession, didReceiveApplicationContext context: [String: Any]) {
        let next = Self.read(context)
        Task { @MainActor in apply(next) }
    }

    nonisolated func sessionReachabilityDidChange(_ session: WCSession) {
        let reachable = session.isReachable
        Task { @MainActor in
            self.reachable = reachable
            if reachable { ask() }
        }
    }
}

/// The song playing: its cover behind, its name and artist, how far it is, and the buttons. The
/// crown changes the iPhone's volume, as on Wear OS.
struct NowPlayingView: View {
    @Environment(PhoneLink.self) private var phone
    @Environment(\.scenePhase) private var phase

    var body: some View {
        let now = phone.now
        ZStack {

            if let cover = phone.cover {
                Image(uiImage: cover).resizable().scaledToFill().blur(radius: 18).opacity(0.55).ignoresSafeArea()
            }
            if now.title.isEmpty {
                VStack(spacing: 6) {
                    Image(systemName: "music.note").font(.title2).foregroundStyle(.tint)
                    Text(phone.reachable ? "Play something in Needle on your iPhone." : "Open Needle on your iPhone.")
                        .font(.footnote).multilineTextAlignment(.center).foregroundStyle(.secondary)
                }
                .padding()
            } else {
                VStack(spacing: 8) {
                    VStack(spacing: 1) {
                        Text(now.title).font(.headline).lineLimit(1)
                        Text(now.artist).font(.footnote).foregroundStyle(.secondary).lineLimit(1)
                    }
                    progress(now)
                    // The iPhone has said nothing for a while: what shows may be old.
                    TimelineView(.periodic(from: .now, by: 5)) { context in
                        if now.stale(at: context.date) {
                            Text(phone.reachable ? "Asking your iPhone…" : "Not up to date").font(.caption2).foregroundStyle(.secondary)
                                .task { phone.ask() }
                        }
                    }
                    HStack(spacing: 14) {
                        button("backward.fill", "Previous") { phone.send("previous") }
                        button(now.playing ? "pause.fill" : "play.fill", now.playing ? "Pause" : "Play", big: true) { phone.send("toggle") }
                        button("forward.fill", "Next") { phone.send("next") }
                    }
                }
                .padding(.horizontal, 6)
            }
        }
        .toolbar {
            if !now.title.isEmpty {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { phone.send("favorite") } label: { Image(systemName: now.favorite ? "heart.fill" : "heart") }
                        .accessibilityLabel(now.favorite ? "Unfavorite" : "Favorite")
                }
            }
        }
        // The system's volume control for the iPhone, small in the corner: it takes the crown
        // while a song shows, and shows the volume while the crown turns.
        .overlay(alignment: .bottomTrailing) {
            if !now.title.isEmpty { PhoneVolume().frame(width: 30, height: 30).accessibilityLabel("iPhone volume") }
        }
        // Opened again: what plays now, not what the iPhone said before.
        .onChange(of: phase) { _, now in if now == .active { phone.ask() } }
    }

    /// A thin line of how far the song is, moving on by itself while it plays, at its speed.
    private func progress(_ now: PhoneLink.NowPlaying) -> some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let played = now.played(at: context.date)
            ProgressView(value: min(max(0, played), max(now.duration, 1)), total: max(now.duration, 1))
                .progressViewStyle(.linear)
                .tint(.accentColor)
        }
        .accessibilityHidden(true)
    }

    private func button(_ symbol: String, _ label: String, big: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(big ? .title2 : .body)
                .frame(width: big ? 50 : 38, height: big ? 50 : 38)
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.circle)
        .tint(big ? .accentColor : nil)
        .accessibilityLabel(label)
    }
}

/// The system's volume control for the paired iPhone (WatchKit's, as Apple's own Now Playing
/// has): the Digital Crown turns the iPhone's volume up and down.
private struct PhoneVolume: WKInterfaceObjectRepresentable {
    func makeWKInterfaceObject(context: Context) -> WKInterfaceVolumeControl {
        WKInterfaceVolumeControl(origin: .companion)
    }

    func updateWKInterfaceObject(_ control: WKInterfaceVolumeControl, context: Context) {
        control.focus()
    }
}
