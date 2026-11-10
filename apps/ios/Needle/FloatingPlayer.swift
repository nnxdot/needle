import SwiftUI

/// In a wide window, the player floats over the foot of the page, as on the Mac (NEEDLE-2.0.md,
/// rule 1): one rounded bar of glass, at most 820 points wide, tinted by the cover as Home's
/// card is; the page runs on under it and fades softly there.
struct FloatingPlayerBar: ViewModifier {
    @Environment(Store.self) private var store
    /// A wide window, where the bar takes the tab bar accessory's place.
    let shown: Bool
    /// The tab open: its bar is the one the player zooms from.
    let source: Bool
    let space: Namespace.ID

    func body(content: Content) -> some View {
        content.safeAreaBar(edge: .bottom, spacing: 0) {
            if shown && store.playingID != nil {
                FloatingPlayer()
                    .modifier(PlayerSource(on: source, space: space))
                    .padding(.horizontal, 24)
                    .padding(.bottom, 14)
                    .transition(Motion.transition(.move(edge: .bottom).combined(with: .opacity)))
            }
        }
        .animation(Motion.fade(.smooth(duration: 0.35)), value: store.playingID == nil)
    }
}

/// The bar the player zooms out of: only the open tab's, so there is one source at a time.
private struct PlayerSource: ViewModifier {
    let on: Bool
    let space: Namespace.ID

    func body(content: Content) -> some View {
        if on { content.matchedTransitionSource(id: "player", in: space) } else { content }
    }
}

/// The floating player: Previous, Play/Pause, and Next; a small screen with the cover, the song,
/// "artist · album", and a thin line along its foot for where it is; then Lyrics, Up Next, and
/// AirPlay. The screen opens the player; Lyrics and Up Next open it with that pane beside the
/// cover.
struct FloatingPlayer: View {
    @Environment(Store.self) private var store
    /// Presses of Play/Pause here: the buzz is for them only.
    @State private var presses = 0
    /// The playing song's timed lines, while Lyrics in the mini player is on.
    @State private var lines: [LyricLine] = []

    var body: some View {
        let song = store.playback?.current
        let playing = store.isPlaying
        let colors = store.colors
        HStack(spacing: 6) {
            HStack(spacing: 2) {
                sign(store.app.sideButtons == "jump" ? "gobackward.10" : "backward.fill", "Previous") {
                    if store.app.sideButtons == "jump" { store.jumpBy(-10) } else { store.previous() }
                }
                Button { presses += 1; store.toggle() } label: {
                    Image(systemName: playing ? "pause.fill" : "play.fill")
                        .font(.system(size: 22))
                        .contentTransition(.symbolEffect(.replace))
                        .frame(width: 44, height: 44)
                        .contentShape(.rect)
                }
                .accessibilityLabel(playing ? "Pause" : "Play")
                .sensoryFeedback(.impact(weight: .light), trigger: presses) { _, _ in store.app.haptics }
                .hoverEffect(.highlight)
                sign(store.app.sideButtons == "jump" ? "goforward.10" : "forward.fill", "Next") {
                    if store.app.sideButtons == "jump" { store.jumpBy(10) } else { store.next() }
                }
            }
            Button { store.showPlayer = true } label: {
                HStack(spacing: 10) {
                    Artwork(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), size: 120)
                        .frame(width: 38, height: 38)
                        .clipShape(.soft(Corner.smallCover))
                        .overlay(alignment: .bottomTrailing) {
                            if store.onComputer {
                                Image(systemName: "laptopcomputer").font(.system(size: 9, weight: .bold))
                                    .padding(3).background(.tint, in: .circle).offset(x: 4, y: 4)
                            }
                        }
                    VStack(alignment: .leading, spacing: 1) {
                        Text(song?.title ?? "").font(.needle(.subheadline).weight(.semibold)).lineLimit(1)
                        if store.app.miniPlayerLyrics, !lines.isEmpty {
                            MiniLyricLine(lines: lines)
                        } else {
                            Text(store.onComputer ? "On your computer" : [song?.artist ?? "", song?.album ?? ""].filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .hoverEffect(.highlight)
            .overlay(alignment: .bottom) { BarProgress().padding(.horizontal, 10) }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(song.map { "\($0.title), \($0.artist)" } ?? "")
            .accessibilityHint("Opens the player")
            HStack(spacing: 2) {
                sign("quote.bubble", "Lyrics") { open("lyrics") }
                sign("list.bullet", "Up Next") { open("queue") }
                RoutePicker(tint: UIColor(colors.text)).frame(width: 44, height: 44).opacity(0.8)
            }
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 10)
        .frame(height: 60)
        .frame(maxWidth: 820)
        // One layer of glass, in the playing cover's colour as Home's card (the accent until it
        // is known); the signs on it are plain.
        .glassEffect(.regular.tint((store.coverColor ?? colors.accent).opacity(0.22)).interactive(), in: .capsule)
        .task(id: "\(store.playingID ?? "")|\(store.lyricsVersion)|\(store.app.miniPlayerLyrics)") { await loadLines() }
    }

    private func sign(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17, weight: .medium))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .accessibilityLabel(label)
        .hoverEffect(.highlight)
        .disabled(store.playback?.current == nil)
    }

    /// Opens the player with the lyrics or the queue in its pane.
    private func open(_ pane: String) {
        store.openPlayer(with: pane)
    }

    private func loadLines() async {
        lines = []
        guard store.app.miniPlayerLyrics, let song = store.playback?.current else { return }
        let found: Lyrics?
        if song.onComputer, let pc = store.pc?.current {
            found = await store.load { $0.pcLyrics(song: pc) } ?? nil
        } else {
            let id = song.id
            found = await store.load { try $0.lyrics(id: id) } ?? nil
        }
        guard !Task.isCancelled, song.id == store.playback?.current?.id, let found, !found.instrumental else { return }
        lines = found.lines
    }
}

/// Where the song is, as a thin line along the foot of the bar's screen: apart, so only it
/// follows the time.
private struct BarProgress: View {
    @Environment(Store.self) private var store

    var body: some View {
        let duration = store.playback?.current?.duration ?? 0
        let part = duration > 0 ? min(1, max(0, store.position / duration)) : 0
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(.primary.opacity(0.12))
                Capsule().fill(.primary.opacity(0.6)).frame(width: geo.size.width * part)
            }
        }
        .frame(height: 2.5)
        .accessibilityHidden(true)
        .allowsHitTesting(false)
    }
}
