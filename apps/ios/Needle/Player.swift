import AVKit
import MediaPlayer
import SwiftUI

/// The song playing, on the tab bar: tap it for the full player. When the tab bar is small
/// (scrolled down), only the song's name and play or pause are left. With Lyrics under the
/// mini player on, the line being sung shows under the song's name in place of its artist.
struct MiniPlayer: View {
    @Environment(Store.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement
    /// The playing song's timed lines, while Lyrics under the mini player is on.
    @State private var lines: [LyricLine] = []

    var body: some View {
        let song = store.playback?.current
        let playing = store.playback?.playing == true
        HStack(spacing: 10) {
            Artwork(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), size: 120)
                .frame(width: 34, height: 34)
                .clipShape(.circle)
                .overlay(alignment: .bottomTrailing) {
                    if store.onComputer {
                        Image(systemName: "laptopcomputer").font(.system(size: 9, weight: .bold))
                            .padding(3).background(.tint, in: .circle).offset(x: 4, y: 4)
                    }
                }
            VStack(alignment: .leading, spacing: 0) {
                Text(song?.title ?? "Not Playing").font(.subheadline.weight(.semibold)).lineLimit(1)
                if placement != .inline, let song {
                    if let sung {
                        // Fades in at each new line; a faint ♪ while none is sung.
                        Text(sung.text.isEmpty ? "♪" : sung.text)
                            .font(.caption.weight(.medium))
                            .foregroundStyle(sung.text.isEmpty ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.primary))
                            .lineLimit(1)
                            .id(sung.index)
                            .transition(.opacity)
                    } else {
                        Text(store.onComputer ? "On your computer" : song.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            }
            .animation(.easeOut(duration: 0.35), value: sung?.index)
            .frame(maxWidth: .infinity, alignment: .leading)
            Button { store.toggle() } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.title3)
                    .contentTransition(.symbolEffect(.replace))
                    .frame(width: 36, height: 36)
            }
            .disabled(song == nil)
            .sensoryFeedback(.impact(weight: .light), trigger: playing) { _, _ in store.app.haptics }
            if placement != .inline {
                Button { store.next() } label: {
                    Image(systemName: "forward.fill").font(.title3).frame(width: 36, height: 36)
                }
                .disabled(song == nil)
            }
        }
        .buttonStyle(.plain)
        .padding(.leading, 6)
        .padding(.trailing, 4)
        .task(id: "\(store.playback?.current?.id ?? "")|\(store.lyricsVersion)|\(store.app.miniPlayerLyrics)") { await loadLines() }
    }

    /// The line being sung (-1 before the first), when the song has timed lyrics and the
    /// setting is on.
    private var sung: (index: Int, text: String)? {
        guard store.app.miniPlayerLyrics, !lines.isEmpty else { return nil }
        let position = store.playback?.position ?? 0
        guard let i = lines.lastIndex(where: { $0.time <= position + 0.2 }) else { return (-1, "") }
        return (i, lines[i].text.trimmingCharacters(in: .whitespaces))
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

/// The full player: the cover over its own colours, the seek bar, the controls, and, a tap away,
/// the lyrics and the songs up next.
struct NowPlayingView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    @State private var pane = Pane.cover
    @State private var favorite = false
    @State private var favoriteTaps = 0
    /// A loop just set, shown until the core plays it (a moment later).
    @State private var pendingLoop: [Double]?

    /// The A-B loop the core plays: it ends it itself (another song, the song chosen again).
    private var loop: [Double] {
        if let start = store.playback?.loopStart, let end = store.playback?.loopEnd { return [start, end] }
        return pendingLoop ?? []
    }
    @State private var swipe: CGFloat = 0

    enum Pane { case cover, lyrics, queue }

    private var reduceMotion: Bool { systemReduceMotion || store.app.reduceMotion }

    var body: some View {
        let song = store.playback?.current
        let seed = (song?.album ?? "") + (song?.artist ?? "")
        let app = store.app
        let immersive = app.immersiveCover && pane == .cover
        ZStack {
            Color.black.ignoresSafeArea()
            if app.immersiveCover {
                // Always there, blurred behind the lyrics and the queue, so moving between them
                // never shows black while it fades.
                ImmersiveCover(song: song, seed: seed, animated: app.animatedCovers && !reduceMotion && pane == .cover)
                if !immersive {
                    Group {
                        if app.coverBackdrop {
                            CoverBackdrop(path: song?.artwork, seed: seed, blur: CGFloat(app.backdropBlur), moving: app.movingBackdrop && !reduceMotion)
                        } else {
                            LinearGradient(colors: [(store.coverColor ?? .gray).opacity(0.55), .black], startPoint: .top, endPoint: .bottom)
                                .background(.black)
                                .ignoresSafeArea()
                        }
                    }
                    .transition(.opacity)
                }
            } else if app.coverBackdrop {
                CoverBackdrop(path: song?.artwork, seed: seed, blur: CGFloat(app.backdropBlur), moving: app.movingBackdrop && !reduceMotion)
            } else {
                LinearGradient(colors: [(store.coverColor ?? .gray).opacity(0.55), .black], startPoint: .top, endPoint: .bottom).ignoresSafeArea()
            }
            VStack(spacing: 0) {
                top(song)
                switch pane {
                case .cover:
                    if immersive {
                        // The cover is the page itself: the words sit over its foot.
                        Spacer(minLength: 8)
                            .contentShape(.rect)
                            .gesture(swipeToSkip)
                    } else {
                        Spacer(minLength: 8)
                        cover(song, seed: seed)
                        Spacer(minLength: 16)
                    }
                    title(song)
                case .lyrics:
                    small(song, seed: seed)
                    LyricsPane(song: song)
                case .queue:
                    small(song, seed: seed)
                    QueuePane()
                }
                SeekBar(duration: song?.duration ?? 0, loop: loop).padding(.top, 16)
                controls
                if store.onComputer {
                    ComputerVolume().padding(.top, 6)
                } else if app.volumeSlider {
                    VolumeBar().frame(height: 34).padding(.top, 6)
                }
                bottom.padding(.top, 14)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 8)
        }
        .animation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.85), value: pane)
        .task(id: song?.id) {
            pendingLoop = nil
            // The new song's star at once, with no empty star between.
            await readFavorite(reset: true)
        }
        .onChange(of: store.playback?.loopStart) { _, _ in pendingLoop = nil }
        // Stars changed elsewhere (the Rate menu): read again, with no blink.
        .task(id: store.ratingVersion) { await readFavorite() }
        .onChange(of: song == nil) { _, gone in if gone { dismiss() } }
        .onChange(of: pane) { _, now in
            UIApplication.shared.isIdleTimerDisabled = now == .lyrics && app.keepScreenOnLyrics
        }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
        #if DEBUG
        .onAppear {
            if CommandLine.arguments.contains("-show-lyrics") { pane = .lyrics }
            if CommandLine.arguments.contains("-show-queue") { pane = .queue }
        }
        #endif
        // The player is always dark: its page is the cover's own dark colours, whatever the
        // theme (light text and glass on them, and a light status bar), and its signs white.
        .tint(.white)
        .preferredColorScheme(.dark)
    }

    /// The top, as Apple Music's: a grabber to tap or pull down to close the player (and, on
    /// the computer, a word that it plays there).
    private func top(_ song: Song?) -> some View {
        VStack(spacing: 6) {
            Capsule().fill(.white.opacity(0.4)).frame(width: 38, height: 5)
            if store.onComputer {
                Text("Playing on your computer").font(.caption.weight(.semibold)).foregroundStyle(.white.opacity(0.7))
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: 34)
        .contentShape(.rect)
        .onTapGesture { dismiss() }
        .gesture(DragGesture(minimumDistance: 10).onEnded { g in
            if g.translation.height > 80 { dismiss() }
        })
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Close the player")
        .padding(.top, 4)
    }

    /// The song's menu, a plain sign beside its name.
    private func moreMenu(_ song: Song?) -> some View {
        Menu {
            if let song, !song.onComputer {
                SongMenu(song: song, inPlayer: true)
                Divider()
                loopMenu(song)
            }
            if !store.onComputer { SleepMenu() }
            Button("Play on…", systemImage: "hifispeaker.2") { store.sheet = .speakers }
        } label: {
            Image(systemName: "ellipsis").font(.title3.weight(.semibold))
                .frame(width: 36, height: 36)
                .contentShape(.circle)
        }
        .foregroundStyle(.white)
    }

    /// A-B loop: repeat part of the song.
    @ViewBuilder
    private func loopMenu(_ song: Song) -> some View {
        Menu {
            // Not in the song's last second: the part would have no length.
            let at = min(store.playback?.position ?? 0, max(0, song.duration - 1))
            Button("Start here (\(clock(at)))") { setLoop(at, loop.count == 2 ? max(loop[1], at + 1) : min(song.duration, at + 30)) }
            if loop.count == 2 {
                Button("End here (\(clock(at)))") { setLoop(loop[0], max(loop[0] + 1, at)) }
                Button("Stop repeating", role: .destructive) { store.core.setLoop(start: nil, end: nil); pendingLoop = nil }
            }
        } label: {
            Label(loop.count == 2 ? "Repeating \(clock(loop[0]))–\(clock(loop[1]))" : "Repeat a part", systemImage: "repeat")
        }
    }

    /// The core takes the loop a moment later: the menu and the seek bar show it at once.
    private func setLoop(_ start: Double, _ end: Double) {
        guard end > start else { return }
        store.core.setLoop(start: start, end: end)
        pendingLoop = [start, end]
    }

    private func cover(_ song: Song?, seed: String) -> some View {
        let playing = store.playback?.playing == true
        let round = store.app.coverShape == "round"
        return RecordSpin(on: round && playing && !reduceMotion) {
            Artwork(path: song?.artwork, seed: seed, size: 1000)
                .aspectRatio(1, contentMode: .fit)
                .clipShape(round ? AnyShape(.circle) : AnyShape(.rect(cornerRadius: 18)))
                .overlay { if round { Circle().fill(.black).frame(width: 26).overlay(Circle().stroke(.white.opacity(0.3), lineWidth: 2)) } }
        }
        .shadow(color: .black.opacity(0.5), radius: 30, y: 20)
        .scaleEffect(playing || round ? 1 : 0.84)
        .offset(x: swipe)
        .animation(reduceMotion ? nil : .spring(response: 0.55, dampingFraction: 0.62), value: playing)
        .frame(maxHeight: 360)
        .gesture(store.app.swipeCover ? DragGesture(minimumDistance: 20)
            .onChanged { g in if abs(g.translation.width) > abs(g.translation.height) { swipe = g.translation.width * 0.6 } }
            .onEnded { g in
                if g.translation.width < -80 { store.next() } else if g.translation.width > 80 { store.previous() }
                withAnimation(.spring(response: 0.4, dampingFraction: 0.75)) { swipe = 0 }
            } : nil)
    }

    private func title(_ song: Song?) -> some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "").font(.title3.bold()).lineLimit(1)
                artistButton(song).font(.title3)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !(song?.onComputer ?? false) { favoriteButton(song) }
            moreMenu(song)
        }
    }

    /// The artist's name, which opens the artist's page.
    @ViewBuilder
    private func artistButton(_ song: Song?) -> some View {
        if let song, !song.onComputer, !isPlaceholderArtist(song.artist) {
            Button { store.opening = .artist(song.artist) } label: {
                Text(song.artist).foregroundStyle(.secondary).lineLimit(1)
            }
            .buttonStyle(.plain)
        } else {
            Text(song?.artist ?? "").foregroundStyle(.secondary).lineLimit(1)
        }
    }

    /// Sideways on the cover: the next or the last song.
    private var swipeToSkip: some Gesture {
        DragGesture(minimumDistance: 20)
            .onEnded { g in
                guard store.app.swipeCover, abs(g.translation.width) > abs(g.translation.height) else { return }
                if g.translation.width < -80 { store.next() } else if g.translation.width > 80 { store.previous() }
            }
    }

    private func readFavorite(reset: Bool = false) async {
        guard let id = store.playback?.current?.id, store.playback?.current?.onComputer == false else {
            if reset { favorite = false }
            return
        }
        let found = await store.load { $0.isFavorite(id: id) } ?? false
        if !Task.isCancelled, store.playback?.current?.id == id, found != favorite { favorite = found }
    }

    private func favoriteButton(_ song: Song?) -> some View {
        Button {
            guard let id = song?.id else { return }
            favorite.toggle()
            if favorite { favoriteTaps += 1 }
            Task {
                if let now = await store.ask({ try $0.toggleFavorite(id: id) }) {
                    if song?.id == store.playback?.current?.id { favorite = now }
                    store.ratingsChanged()
                } else {
                    favorite.toggle()
                }
            }
        } label: {
            // A plain sign, as Apple Music's.
            Image(systemName: favorite ? "star.fill" : "star").font(.title3.weight(.semibold))
                .foregroundStyle(.white)
                .frame(width: 36, height: 36)
                .contentShape(.circle)
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(favorite ? "Unfavourite" : "Favourite")
        // Only for a tap: not when the star is read for a song that is a favourite.
        .sensoryFeedback(.success, trigger: favoriteTaps) { _, _ in store.app.haptics }
    }

    /// The song, small, above the lyrics or the queue.
    private func small(_ song: Song?, seed: String) -> some View {
        HStack(spacing: 14) {
            Artwork(path: song?.artwork, seed: seed, size: 240)
                .frame(width: 72, height: 72)
                .clipShape(.rect(cornerRadius: 10))
                .shadow(color: .black.opacity(0.3), radius: 8, y: 4)
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "").font(.title3.weight(.semibold)).lineLimit(1)
                artistButton(song).font(.body)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !(song?.onComputer ?? false) { favoriteButton(song) }
            moreMenu(song)
        }
        .padding(.vertical, 14)
        .transition(.opacity)
    }

    private var controls: some View {
        let playing = store.playback?.playing == true
        let jump = store.app.sideButtons == "jump"
        return HStack {
            Spacer()
            if jump { BigButton(symbol: "gobackward.10", size: 32) { store.jumpBy(-10) } }
            else { BigButton(symbol: "backward.fill", size: 34) { store.previous() } }
            Spacer()
            BigButton(symbol: playing ? "pause.fill" : "play.fill", size: 46) { store.toggle() }
                .sensoryFeedback(.impact(weight: .medium), trigger: playing) { _, _ in store.app.haptics }
            Spacer()
            if jump { BigButton(symbol: "goforward.10", size: 32) { store.jumpBy(10) } }
            else { BigButton(symbol: "forward.fill", size: 34) { store.next() } }
            Spacer()
        }
        .padding(.top, 10)
    }

    /// Lyrics, speakers, and the queue: plain signs, as Apple Music's, the one open lit.
    private var bottom: some View {
        HStack {
            paneButton(.lyrics, symbol: "quote.bubble")
            Spacer()
            RoutePicker().frame(width: 44, height: 44).opacity(0.75)
            Spacer()
            paneButton(.queue, symbol: "list.bullet")
        }
        .padding(.horizontal, 40)
    }

    private func paneButton(_ which: Pane, symbol: String) -> some View {
        let on = pane == which
        return Button { pane = on ? .cover : which } label: {
            Image(systemName: on && which == .lyrics ? "quote.bubble.fill" : symbol)
                .font(.title3.weight(on ? .semibold : .regular))
                .foregroundStyle(.white.opacity(on ? 1 : 0.7))
                .frame(width: 44, height: 44)
                .background(.white.opacity(on ? 0.2 : 0), in: .circle)
                .contentShape(.circle)
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(which == .lyrics ? "Lyrics" : "Up Next")
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// Turns its content slowly, as a record on a turntable, while `on`.
struct RecordSpin<Content: View>: View {
    let on: Bool
    /// Seconds for one turn.
    var period: Double = 12
    /// Frames a second, at most.
    var rate: Double = 30
    @ViewBuilder let content: () -> Content
    /// When it began turning this time; nil while still.
    @State private var start: Date?
    /// The turn before this time.
    @State private var held: Double = 0

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / rate, paused: !on)) { context in
            // From the state alone, so the frame on which `on` changes does not jump.
            let turned = held + (start.map { max(0, context.date.timeIntervalSince($0)) } ?? 0) * 360 / period
            content().rotationEffect(.degrees(turned))
        }
        .onChange(of: on, initial: true) { _, now in
            if now {
                if start == nil { start = Date() }
            } else if let began = start {
                held += Date().timeIntervalSince(began) * 360 / period
                start = nil
            }
        }
    }
}

/// A large control without a frame: glass shows under it only while it is held.
struct BigButton: View {
    let symbol: String
    let size: CGFloat
    let action: () -> Void
    @State private var held = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size))
                .contentTransition(.symbolEffect(.replace))
                .frame(width: size * 1.9, height: size * 1.9)
                .background {
                    Circle().fill(.white.opacity(held ? 0.16 : 0))
                }
                .contentShape(.circle)
        }
        .buttonStyle(Held(held: $held))
    }

    struct Held: ButtonStyle {
        @Binding var held: Bool
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                .scaleEffect(configuration.isPressed ? 1.12 : 1)
                .animation(.spring(response: 0.3, dampingFraction: 0.55), value: configuration.isPressed)
                .onChange(of: configuration.isPressed) { _, now in
                    withAnimation(.easeOut(duration: 0.2)) { held = now }
                }
        }
    }
}

/// The seek bar: a strip of glass that grows while it is dragged, with the A-B loop marked.
struct SeekBar: View {
    @Environment(Store.self) private var store
    let duration: Double
    var loop: [Double] = []
    @State private var dragged: Double?

    var body: some View {
        let position = dragged ?? store.playback?.position ?? 0
        let part = duration > 0 ? min(1, max(0, position / duration)) : 0
        VStack(spacing: 6) {
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(.white.opacity(0.18))
                    if loop.count == 2, duration > 0 {
                        Capsule().fill(Color.accentColor.opacity(0.5))
                            .frame(width: max(4, geo.size.width * (loop[1] - loop[0]) / duration))
                            .offset(x: geo.size.width * loop[0] / duration)
                    }
                    Capsule().fill(.white.opacity(dragged == nil ? 0.75 : 0.95))
                        .frame(width: max(0, geo.size.width * part))
                }
                .frame(height: dragged == nil ? 7 : 12)
                .frame(maxHeight: .infinity)
                .contentShape(.rect)
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { g in dragged = duration * min(1, max(0, g.location.x / geo.size.width)) }
                        .onEnded { _ in
                            if let dragged { store.seek(dragged) }
                            // The core takes a moment to report the new place.
                            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { self.dragged = nil }
                        }
                )
            }
            .frame(height: 20)
            .animation(.spring(response: 0.3, dampingFraction: 0.7), value: dragged == nil)
            .accessibilityElement()
            .accessibilityLabel("Position")
            .accessibilityValue("\(clock(position)) of \(clock(duration))")
            .accessibilityAdjustableAction { direction in
                switch direction {
                case .increment: store.jumpBy(10)
                case .decrement: store.jumpBy(-10)
                @unknown default: break
                }
            }
            HStack {
                Text(clock(position))
                Spacer()
                if let format = store.playback?.current?.format, !format.isEmpty {
                    Text(format).font(.caption2.weight(.bold)).padding(.horizontal, 6).padding(.vertical, 2)
                        .overlay(Capsule().stroke(.secondary, lineWidth: 1))
                }
                Spacer()
                Text("-" + clock(max(0, duration - position)))
            }
            .font(.caption.weight(.semibold).monospacedDigit())
            .foregroundStyle(.secondary)
        }
    }
}

/// The phone's own volume, as Control Center sets it, drawn as Apple Music's: a thin line
/// between a quiet and a loud speaker, with no knob.
struct VolumeBar: View {
    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "speaker.fill").font(.caption)
            SystemVolume().frame(height: 34)
            Image(systemName: "speaker.wave.3.fill").font(.caption)
        }
        .foregroundStyle(.white.opacity(0.6))
    }
}

/// The system's volume slider (only it may change the phone's volume), in a thin line.
private struct SystemVolume: UIViewRepresentable {
    func makeUIView(context: Context) -> Centred {
        let view = MPVolumeView(frame: .zero)
        view.setVolumeThumbImage(Self.clear, for: .normal)
        view.setMinimumVolumeSliderImage(Self.line(.white.withAlphaComponent(0.85)), for: .normal)
        view.setMaximumVolumeSliderImage(Self.line(.white.withAlphaComponent(0.22)), for: .normal)
        return Centred(view)
    }

    func updateUIView(_ view: Centred, context: Context) {}

    /// The system draws its slider at the top of its frame: this holds it in the middle, in
    /// line with the speakers beside it.
    final class Centred: UIView {
        private let volume: MPVolumeView

        init(_ volume: MPVolumeView) {
            self.volume = volume
            super.init(frame: .zero)
            addSubview(volume)
        }

        required init?(coder: NSCoder) { nil }

        override func layoutSubviews() {
            super.layoutSubviews()
            let height = max(1, volume.sizeThatFits(bounds.size).height)
            volume.frame = CGRect(x: 0, y: (bounds.height - height) / 2, width: bounds.width, height: height)
        }
    }

    /// No knob to see, but one to hold.
    static let clear = UIGraphicsImageRenderer(size: CGSize(width: 28, height: 28)).image { _ in }

    /// A round-ended line, stretched along the slider.
    static func line(_ color: UIColor) -> UIImage {
        UIGraphicsImageRenderer(size: CGSize(width: 14, height: 7)).image { _ in
            color.setFill()
            UIBezierPath(roundedRect: CGRect(x: 0, y: 0, width: 14, height: 7), cornerRadius: 3.5).fill()
        }
        .resizableImage(withCapInsets: UIEdgeInsets(top: 0, left: 4, bottom: 0, right: 4))
    }
}

/// The computer's volume, while the phone works the computer's player.
struct ComputerVolume: View {
    @Environment(Store.self) private var store
    @State private var value: Double?

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "speaker.fill").font(.caption)
            LineSlider(value: value ?? Double(store.pc?.volume ?? 1)) { value = $0 } done: {
                guard let value else { return }
                store.setComputerVolume(Float(value))
                // Until the computer's next answer has it.
                Task {
                    try? await Task.sleep(for: .seconds(1.5))
                    if self.value == value { self.value = nil }
                }
            }
            Image(systemName: "speaker.wave.3.fill").font(.caption)
        }
        .foregroundStyle(.white.opacity(0.6))
    }
}

/// A thin line to drag along, thicker while held, as Apple Music's volume.
struct LineSlider: View {
    let value: Double
    let changed: (Double) -> Void
    var done: () -> Void = {}
    @State private var held = false

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(.white.opacity(0.22))
                Capsule().fill(.white.opacity(0.85)).frame(width: geo.size.width * min(1, max(0, value)))
            }
            .frame(height: held ? 11 : 7)
            .frame(maxHeight: .infinity)
            .contentShape(.rect)
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { g in held = true; changed(min(1, max(0, g.location.x / max(1, geo.size.width)))) }
                .onEnded { _ in held = false; done() })
        }
        .frame(height: 34)
        .animation(.spring(response: 0.3, dampingFraction: 0.7), value: held)
        .accessibilityElement()
        .accessibilityLabel("Volume")
        .accessibilityValue("\(Int((value * 100).rounded())) percent")
        .accessibilityAdjustableAction { direction in
            let step: Double
            switch direction {
            case .increment: step = 0.1
            case .decrement: step = -0.1
            @unknown default: return
            }
            changed(min(1, max(0, value + step)))
            done()
        }
    }
}

/// The player's cover as Apple Music shows it: across the top, edge to edge, fading into the
/// cover's colour (darkened, as the player is always dark); a moving cover plays over it.
struct ImmersiveCover: View {
    @Environment(Store.self) private var store
    let song: Song?
    let seed: String
    let animated: Bool
    @State private var tone: Color
    @State private var video: URL?

    init(song: Song?, seed: String, animated: Bool) {
        self.song = song
        self.seed = seed
        self.animated = animated
        // The colour already known, so it never starts from black.
        _tone = State(initialValue: PageColors.cached(song?.artwork, seed: seed)?.page.darkened(to: 0.3) ?? .black)
    }

    var body: some View {
        GeometryReader { geo in
            let side = geo.size.width
            // Down to just above the song's name and the controls, as in Apple Music: on a tall
            // phone a square cover stopped halfway, over an empty band.
            let tall = max(side, geo.size.height - 400)
            ZStack(alignment: .top) {
                tone
                ZStack {
                    Artwork(path: song?.artwork, seed: seed, size: 1400)
                    if animated, let video { LoopingVideo(url: video) }
                }
                .frame(width: side, height: tall)
                .clipped()
                .overlay(alignment: .bottom) { Fade(color: tone).frame(height: tall * 0.42) }
                // The status bar and the top buttons read over a bright cover.
                LinearGradient(colors: [.black.opacity(0.5), .black.opacity(0.2), .clear], startPoint: .top, endPoint: .bottom)
                    .frame(height: 220)
            }
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 0.5), value: tone)
        .task(id: "\(song?.id ?? "")|\(animated)") {
            video = nil
            let colors = await PageColors.of(song?.artwork, seed: seed)
            guard !Task.isCancelled else { return }
            tone = colors.page.darkened(to: 0.3)
            guard animated, let id = song?.id, !(song?.onComputer ?? true) else { return }
            let found = (await store.load { $0.animatedCover(id: id) } ?? nil).map(URL.init(fileURLWithPath:))
            if !Task.isCancelled { video = found }
        }
    }
}

/// A short video over and over, silent, filling its frame: a moving cover.
struct LoopingVideo: UIViewRepresentable {
    let url: URL

    func makeUIView(context: Context) -> PlayerView {
        let view = PlayerView()
        view.play(url)
        return view
    }

    func updateUIView(_ view: PlayerView, context: Context) {
        if view.url != url { view.play(url) }
    }

    static func dismantleUIView(_ view: PlayerView, coordinator: ()) {
        view.stop()
    }

    final class PlayerView: UIView {
        override class var layerClass: AnyClass { AVPlayerLayer.self }
        private let player = AVQueuePlayer()
        private var looper: AVPlayerLooper?
        private(set) var url: URL?
        private var watching: [NSObjectProtocol] = []

        override init(frame: CGRect) {
            super.init(frame: frame)
            let center = NotificationCenter.default
            // Still while the app is away, on again when it is back.
            watching = [
                center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in self?.player.pause() },
                center.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in
                    if self?.looper != nil { self?.player.play() }
                },
            ]
        }

        required init?(coder: NSCoder) { nil }

        deinit { watching.forEach(NotificationCenter.default.removeObserver) }

        func play(_ url: URL) {
            stop()
            self.url = url
            let layer = layer as! AVPlayerLayer
            layer.player = player
            layer.videoGravity = .resizeAspectFill
            player.isMuted = true
            // A cover, not a video: it never takes the phone's sound or the lock screen.
            player.audiovisualBackgroundPlaybackPolicy = .pauses
            player.preventsDisplaySleepDuringVideoPlayback = false
            looper = AVPlayerLooper(player: player, templateItem: AVPlayerItem(url: url))
            player.play()
        }

        /// The last video goes wholly, so it never shows over the next cover.
        func stop() {
            looper?.disableLooping()
            looper = nil
            player.pause()
            player.removeAllItems()
            url = nil
        }
    }
}

/// AirPlay and Bluetooth speakers.
struct RoutePicker: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = .white
        view.activeTintColor = UIColor(named: "AccentColor")
        view.prioritizesVideoDevices = false
        return view
    }

    func updateUIView(_ view: AVRoutePickerView, context: Context) {}
}

/// Timed lyrics that follow the song (tap a line to go there), with each word lit as it is
/// sung when the lyrics have word timing; or the plain words.
struct LyricsPane: View {
    @Environment(Store.self) private var store
    let song: Song?
    @State private var lyrics: Lyrics?
    @State private var loaded = false
    /// Online lookups are on (LRCLIB and the lyrics plugins).
    @State private var online = true

    var body: some View {
        Group {
            if let lyrics, !lyrics.lines.isEmpty {
                if store.app.liveLyrics { timed(lyrics.lines) } else { plain(lyrics.lines.map(\.text).joined(separator: "\n")) }
            } else if let lyrics, !lyrics.plain.isEmpty {
                plain(lyrics.plain)
            } else if loaded {
                ScrollView { VStack(spacing: 14) {
                    ContentUnavailableView(lyrics?.instrumental == true ? "Instrumental" : "No lyrics for this song", systemImage: "quote.bubble")
                    if !online, lyrics?.instrumental != true {
                        Text("Online lookups are off, so Needle only looks in the song's own files and your music server.")
                            .font(.footnote).foregroundStyle(.secondary).multilineTextAlignment(.center)
                        Button("Look Up Lyrics Online") {
                            Task {
                                _ = await store.ask { core in
                                    var settings = try core.playbackSettings()
                                    settings.onlineMedia = true
                                    try core.setPlaybackSettings(value: settings)
                                }
                                await load()
                            }
                        }
                        // Plain glass: the player's white tint made a prominent one white on white.
                        .buttonStyle(.glass)
                        .fontWeight(.semibold)
                    }
                    if let song, !song.onComputer, lyrics?.instrumental != true {
                        Button("Ask NetEase") {
                            let id = song.id
                            Task {
                                _ = await store.ask { try $0.addNetease(song: id) }
                                await load()
                            }
                        }
                        .buttonStyle(.glass)
                        Button("Add and time them") { store.sheet = .timing(song) }
                            .buttonStyle(.glass)
                    }
                }
                .padding(.vertical, 20)
                .frame(maxWidth: .infinity) }
                .scrollBounceBehavior(.basedOnSize)
            } else {
                ProgressView().frame(maxHeight: .infinity)
            }
        }
        .frame(maxHeight: .infinity)
        .task(id: "\(song?.id ?? "")|\(store.lyricsVersion)") { await load() }
    }

    private func load() async {
        loaded = false
        lyrics = nil
        guard let song else { return }
        let isOnline = await store.load { try $0.playbackSettings().onlineMedia } ?? true
        let found: Lyrics?
        if song.onComputer, let pc = store.pc?.current {
            found = await store.load { $0.pcLyrics(song: pc) } ?? nil
        } else {
            let id = song.id
            found = await store.load { try $0.lyrics(id: id) } ?? nil
        }
        // The song changed while these were looked up: they are the last song's.
        guard !Task.isCancelled, song.id == store.playback?.current?.id else { return }
        online = isOnline
        lyrics = found
        loaded = true
    }

    private func plain(_ text: String) -> some View {
        ScrollView {
            Text(text).font(.title3.weight(.semibold)).frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, 20)
        }
        .mask(fade)
    }

    /// The lyrics fade out at the top and the foot as they scroll.
    private var fade: LinearGradient {
        LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.08), .init(color: .black, location: 0.8), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom)
    }

    private func timed(_ lines: [LyricLine]) -> some View {
        let position = store.playback?.position ?? 0
        let current = lines.lastIndex { $0.time <= position + 0.2 } ?? -1
        return ScrollViewReader { reader in
            ScrollView(showsIndicators: false) {
                LazyVStack(alignment: .leading, spacing: 22) {
                    Color.clear.frame(height: 40)
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        Group {
                            if i == current && !line.words.isEmpty {
                                karaoke(line, at: position)
                            } else {
                                Text(line.text.isEmpty ? "♪" : line.text)
                            }
                        }
                        .font(.system(size: 28, weight: .heavy))
                        .opacity(i == current ? 1 : 0.32)
                        .blur(radius: i == current ? 0 : 1)
                        // Room for the playing line to grow without going past the edge.
                        .padding(.trailing, 16)
                        .scaleEffect(i == current ? 1.03 : 1, anchor: .leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .id(i)
                        .onTapGesture { store.seek(line.time) }
                    }
                    Color.clear.frame(height: 200)
                }
                .animation(.spring(response: 0.5, dampingFraction: 0.8), value: current)
            }
            .onChange(of: current, initial: true) { _, now in
                guard now >= 0 else { return }
                withAnimation(.spring(response: 0.6, dampingFraction: 0.9)) { reader.scrollTo(now, anchor: UnitPoint(x: 0, y: 0.25)) }
            }
        }
        .mask(fade)
    }

    /// The line playing, its words lit one by one as they are sung.
    private func karaoke(_ line: LyricLine, at position: Double) -> Text {
        line.words.reduce(Text("")) { text, word in
            Text("\(text)\(Text(word.text).foregroundStyle(word.time <= position ? Color.white : Color.white.opacity(0.4)))")
        }
    }
}

/// The songs up next: tap one to play it, swipe to take it out, drag to move it.
struct QueuePane: View {
    @Environment(Store.self) private var store

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 12) {
                Button { store.shuffle() } label: { Pill(symbol: "shuffle", on: false) }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Shuffle")
                Button { store.cycleRepeat() } label: {
                    Pill(symbol: store.playback?.repeat == .one ? "repeat.1" : "repeat", on: store.playback?.repeat != .off)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Repeat")
                .accessibilityValue(store.playback?.repeat == .one ? "One" : store.playback?.repeat == .all ? "All" : "Off")
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(store.onComputer ? "Up Next on Your Computer" : "Continue Playing").font(.headline)
                if let album = store.playback?.current?.album, !album.isEmpty {
                    Text("From \(album)").font(.subheadline).foregroundStyle(.white.opacity(0.6)).lineLimit(1)
                }
            }
            QueueList()
        }
        .frame(maxHeight: .infinity)
    }
}

/// The songs up next, apart from the pills and the title, so the list is made again only when
/// the queue changes.
private struct QueueList: View {
    @Environment(Store.self) private var store

    var body: some View {
        Group {
            if store.upNext.isEmpty {
                ContentUnavailableView("Nothing up next", systemImage: "list.bullet").frame(maxHeight: .infinity)
            } else {
                List {
                    ForEach(Array(store.upNext.enumerated()), id: \.offset) { i, song in
                        Button { store.jump(i) } label: {
                            HStack(spacing: 14) {
                                Artwork(path: song.artwork, seed: song.album + song.artist, size: 150)
                                    .frame(width: 50, height: 50).clipShape(.rect(cornerRadius: 6))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(song.title).lineLimit(1)
                                    Text(song.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                                }
                                Spacer()
                            }
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(Color.clear)
                        .listRowSeparator(.hidden)
                        .listRowInsets(EdgeInsets(top: 6, leading: 0, bottom: 6, trailing: 0))
                    }
                    .onDelete(perform: store.onComputer ? nil : { rows in store.removeUpNext(at: rows) })
                    .onMove(perform: store.onComputer ? nil : { from, to in store.moveUpNext(from: from, to: to) })
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
            }
        }
    }
}

/// A wide pill of Up Next's (shuffle, repeat), as Apple Music's: soft, light while on.
struct Pill: View {
    let symbol: String
    let on: Bool

    var body: some View {
        Image(systemName: symbol)
            .font(.body.weight(.semibold))
            .foregroundStyle(on ? Color.black.opacity(0.8) : .white)
            .frame(maxWidth: .infinity)
            .frame(height: 40)
            .background(.white.opacity(on ? 0.75 : 0.16), in: .capsule)
            .contentShape(.capsule)
            .contentTransition(.symbolEffect(.replace))
    }
}

/// A player's sign without a frame, as Apple Music's: white, a soft circle behind it while on.
struct PlainSign: View {
    let symbol: String
    let on: Bool

    var body: some View {
        Image(systemName: symbol)
            .font(.body.weight(.semibold))
            .foregroundStyle(on ? Color.black : .white.opacity(0.85))
            .frame(width: 34, height: 34)
            .background(.white.opacity(on ? 0.9 : 0.12), in: .circle)
            .contentShape(.circle)
            .contentTransition(.symbolEffect(.replace))
    }
}

/// Lets a button take one of two glass styles.
struct AnyPrimitiveButtonStyle: PrimitiveButtonStyle {
    private let make: (Configuration) -> AnyView

    init<S: PrimitiveButtonStyle>(_ style: S) {
        make = { AnyView(style.makeBody(configuration: $0)) }
    }

    func makeBody(configuration: Configuration) -> some View { make(configuration) }
}

// MARK: - Speakers and lyrics timing

/// Chromecast, DLNA, and AirPlay speakers on the network: one, or several at once, lined up.
struct SpeakersView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var all: [Output]?
    @State private var chosen = Set<String>()
    @State private var phone = true
    @State private var delays: [String: Int32] = [:]

    var body: some View {
        Form {
            Section {
                Toggle(isOn: $phone) { Label("This iPhone", systemImage: "iphone") }
                if let all {
                    ForEach(all, id: \.id) { o in
                        Toggle(isOn: Binding(get: { chosen.contains(o.id) }, set: { if $0 { chosen.insert(o.id) } else { chosen.remove(o.id) } })) {
                            Label {
                                Text(o.name)
                                Text(o.kind)
                            } icon: { Image(systemName: o.kind == "Chromecast" ? "tv" : "hifispeaker") }
                        }
                    }
                    if all.isEmpty {
                        Text("No speakers found. Chromecast, DLNA, and AirPlay speakers on the same Wi-Fi show here.").foregroundStyle(.secondary)
                    }
                } else {
                    HStack { ProgressView(); Text("Looking for speakers on your network…") }
                }
            } footer: {
                Text("Choose one, or several to play on all at once. For AirPlay and Bluetooth, the AirPlay button in the player works too.")
            }
            let playing = Array(chosen) + (phone ? [""] : [])
            if playing.count > 1 {
                Section {
                    ForEach(playing, id: \.self) { id in
                        VStack(alignment: .leading) {
                            LabeledContent(id.isEmpty ? "This iPhone" : all?.first { $0.id == id }?.name ?? "Speaker", value: "\(delays[id] ?? 0) ms")
                            Slider(value: Binding(get: { Double(delays[id] ?? 0) }, set: { delays[id] = Int32(($0 / 10).rounded() * 10) }), in: 0...1000)
                        }
                    }
                } header: {
                    Text("Line them up")
                } footer: {
                    Text("Hold back any that plays early, until they sound as one.")
                }
            }
        }
        .navigationTitle("Play on")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            ToolbarItem(placement: .confirmationAction) { Button("Play") { apply() } }
        }
        .task {
            guard let found = await store.load({ $0.speakers() }) else { all = []; return }
            chosen = Set(found.chosen)
            phone = found.thisPhone
            delays = found.delays
            all = found.all
        }
    }

    private func apply() {
        let ids = Array(chosen)
        let group = ids.count + (phone ? 1 : 0) > 1
        let (here, wait) = (phone || ids.isEmpty, group ? delays : [:])
        store.run { try $0.setSpeakers(ids: ids, thisPhone: here, delays: wait) }
        store.message = ids.isEmpty ? "Playing on this iPhone" : group ? "Playing on \(ids.count + (phone ? 1 : 0)) at once" : "Playing on \(all?.first { $0.id == ids[0] }?.name ?? "the speaker")"
        dismiss()
    }
}

/// Times a song's lyrics by tapping as each line starts.
struct TimingView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let song: Song
    @State private var lines: [LyricLine] = []
    @State private var next = 0
    @State private var text = ""
    @State private var loaded = false

    var body: some View {
        let thisSong = store.playback?.current?.id == song.id
        VStack(spacing: 0) {
            List {
                Section {
                    Text(!thisSong ? "Play this song, then tap as each line starts." : lines.isEmpty ? "This song has no lyrics to time. Paste them below, one line per line." : "Tap the big button as each line starts. Tap a line to go back to it.")
                        .font(.subheadline).foregroundStyle(.secondary)
                    if !thisSong {
                        Button("Play it from the start", systemImage: "play.fill") { store.play([song]) }
                    }
                }
                if lines.isEmpty && loaded {
                    Section {
                        TextEditor(text: $text).frame(minHeight: 160)
                        Button("Use these lines") {
                            lines = text.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                                .map { LyricLine(time: -1, text: $0, words: []) }
                        }
                    }
                }
                ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                    Button { next = i } label: {
                        HStack {
                            Text(line.time >= 0 ? clock(line.time) : "–").font(.subheadline.monospacedDigit()).foregroundStyle(.tint).frame(width: 52, alignment: .leading)
                            Text(line.text).fontWeight(i == next ? .bold : .regular).foregroundStyle(.primary)
                        }
                    }
                    .listRowBackground(i == next ? Color.accentColor.opacity(0.2) : Color.clear)
                    .id(i)
                }
            }
            HStack(spacing: 12) {
                Button {
                    guard next < lines.count else { return }
                    lines[next].time = store.playback?.position ?? 0
                    next += 1
                } label: {
                    Text(next < lines.count ? "Tap: line \(next + 1) starts" : "All timed").frame(maxWidth: .infinity).padding(.vertical, 10)
                }
                .buttonStyle(.glassProminent)
                .disabled(!thisSong || next >= lines.count)
                .sensoryFeedback(.impact(weight: .light), trigger: next)
                Button("Save") {
                    let (id, timed) = (song.id, lines.sorted { $0.time < $1.time })
                    Task {
                        if await store.ask({ try $0.saveTimedLyrics(id: id, lines: timed) }) != nil {
                            store.lyricsChanged()
                            store.message = "Saved the timed lyrics"
                            dismiss()
                        }
                    }
                }
                .buttonStyle(.glass)
                .disabled(lines.isEmpty || lines.contains { $0.time < 0 })
            }
            .padding()
        }
        .navigationTitle("Time the lyrics")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            ToolbarItem(placement: .destructiveAction) {
                Menu {
                    Button("Forget my timing", role: .destructive) {
                        store.core.deleteTimedLyrics(id: song.id)
                        store.lyricsChanged()
                        store.message = "Timing taken off"
                        dismiss()
                    }
                } label: { Image(systemName: "ellipsis") }
            }
        }
        .task {
            let id = song.id
            let found = await store.load { try $0.lyrics(id: id) } ?? nil
            if let found {
                lines = !found.lines.isEmpty ? found.lines
                    : found.plain.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                        .map { LyricLine(time: -1, text: $0, words: []) }
            }
            loaded = true
        }
    }
}
