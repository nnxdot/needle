import AVKit
import MediaPlayer
import SwiftUI
import Translation

/// The song playing, on the tab bar: tap it for the full player. When the tab bar is small
/// (scrolled down), only the song's name and play or pause are left. With Lyrics in the mini
/// player on, the line being sung shows under the song's name in place of its artist.
struct MiniPlayer: View {
    @Environment(Store.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement
    /// Opens the full player.
    let open: () -> Void
    /// The playing song's timed lines, while Lyrics in the mini player is on.
    @State private var lines: [LyricLine] = []
    /// Presses of Play/Pause here: the buzz is for them, not for a song that ends.
    @State private var presses = 0

    var body: some View {
        let song = store.playback?.current
        let playing = store.isPlaying
        HStack(spacing: 10) {
            Button(action: open) {
                HStack(spacing: 10) {
                    Artwork(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), size: 120)
                        .frame(width: 34, height: 34)
                        .clipShape(.soft(Corner.smallCover))
                        .overlay(alignment: .bottomTrailing) {
                            if store.onComputer {
                                Image(systemName: "laptopcomputer").font(.system(size: 9, weight: .bold))
                                    .padding(3).background(.tint, in: .circle).offset(x: 4, y: 4)
                            }
                        }
                    VStack(alignment: .leading, spacing: 0) {
                        Text(song?.title ?? "Not Playing").font(.needle(.subheadline).weight(.semibold)).lineLimit(1)
                        if placement != .inline, let song {
                            if store.app.miniPlayerLyrics, !lines.isEmpty {
                                MiniLyricLine(lines: lines)
                            } else {
                                Text(store.onComputer ? "On your computer" : song.artist).font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1)
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .contentShape(.rect)
            }
            .disabled(song == nil)
            .accessibilityElement(children: .combine)
            .accessibilityLabel(song.map { "\($0.title), \($0.artist)" } ?? "Not Playing")
            .accessibilityHint("Opens the player")
            Button {
                presses += 1
                store.toggle()
            } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.needle(.title3))
                    .contentTransition(.symbolEffect(.replace))
                    .frame(width: 44, height: 44)
                    .contentShape(.rect)
            }
            .disabled(song == nil)
            .accessibilityLabel(playing ? "Pause" : "Play")
            .sensoryFeedback(.impact(weight: .light), trigger: presses) { _, _ in store.app.haptics }
            if placement != .inline {
                Button { store.next() } label: {
                    Image(systemName: "forward.fill").font(.needle(.title3)).frame(width: 44, height: 44).contentShape(.rect)
                }
                .disabled(song == nil)
                .accessibilityLabel("Next")
            }
        }
        .buttonStyle(.plain)
        .padding(.leading, 6)
        .padding(.trailing, 2)
        .task(id: "\(store.playingID ?? "")|\(store.lyricsVersion)|\(store.app.miniPlayerLyrics)") { await loadLines() }
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

/// The line being sung, under the song's name in the mini player: apart, so only it follows
/// the time. Fades in at each new line; a faint ♪ while none is sung.
struct MiniLyricLine: View {
    @Environment(Store.self) private var store
    let lines: [LyricLine]

    var body: some View {
        let position = store.position
        let index = lines.lastIndex(where: { $0.time <= position + 0.2 }) ?? -1
        let text = index >= 0 ? lines[index].text.trimmingCharacters(in: .whitespaces) : ""
        ZStack(alignment: .leading) {
            Text(text.isEmpty ? "♪" : text)
                .font(.needle(.caption).weight(.medium))
                .foregroundStyle(text.isEmpty ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.primary))
                .lineLimit(1)
                .id(index)
                // The old line blurs away as the new one comes clear, in 0.2 s: never two
                // sharp lines over each other.
                .transition(Motion.transition(AnyTransition(.blurReplace)))
        }
        .animation(Motion.fade(.easeOut(duration: 0.2)), value: index)
    }
}

/// Room at the top for the status bar where the safe area does not leave it (an iPad on its
/// side gives only its corners' room): for Needle's own controls at the top, not a system bar's.
struct ClearsStatusBar: ViewModifier {
    @State private var bar: CGFloat = 0
    @State private var safe: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .padding(.top, max(0, bar - safe))
            .onGeometryChange(for: CGFloat.self) { $0.safeAreaInsets.top } action: { safe = $0 }
            .background {
                WindowReader(found: { window in
                    let found = window.windowScene?.statusBarManager?.statusBarFrame.height ?? 0
                    if found != bar { bar = found }
                }, becameKey: {})
                .frame(width: 0, height: 0)
            }
    }
}

/// The pane beside the cover in a wide window, as the Mac's side panel: Lyrics and Up Next as
/// its tabs, on one floating pane of glass in the cover's colour (22-point corners). What is on
/// it is plain, never glass on glass.
struct PlayerSidePane: View {
    @Environment(Store.self) private var store
    @Binding var side: String
    let song: Song?
    /// Up next's Add songs, as on the phone's sheet.
    let addSongs: () -> Void
    @AppStorage("translateLyrics") private var translate = false
    @Namespace private var tabs

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 4) {
                tab("Lyrics", "lyrics")
                tab("Up Next", "queue")
                Spacer(minLength: 8)
                if side == "lyrics" {
                    Button { withFade(.smooth(duration: 0.3)) { translate.toggle() } } label: {
                        Image(systemName: "translate")
                            .font(.needle(.body).weight(translate ? .semibold : .regular))
                            .foregroundStyle(.white.opacity(translate ? 1 : 0.7))
                            .frame(width: 36, height: 36)
                            .background(.white.opacity(translate ? 0.18 : 0), in: .circle)
                            .contentShape(.circle)
                    }
                    .buttonStyle(Pressable.sign)
                    .hoverEffect(.highlight)
                    .accessibilityLabel(translate ? "Hide the translation" : "Translate the lyrics")
                    .accessibilityAddTraits(translate ? .isSelected : [])
                }
                Button { withMotion(.smooth(duration: 0.3)) { side = "" } } label: {
                    Image(systemName: "xmark").font(.needle(.subheadline).weight(.semibold))
                        .foregroundStyle(.white.opacity(0.7))
                        .frame(width: 36, height: 36)
                        .contentShape(.circle)
                }
                .buttonStyle(Pressable.sign)
                .hoverEffect(.highlight)
                .accessibilityLabel("Close the side pane")
            }
            Group {
                if side == "queue" {
                    QueuePane(onPane: true, titled: false, addSongs: addSongs)
                } else {
                    LyricsPane(song: song, onPane: true)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .transition(.opacity)
        }
        .padding(16)
        .frame(maxHeight: .infinity)
        .glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.18)), in: .soft(Corner.large))
        .animation(Motion.fade(.easeInOut(duration: 0.25)), value: side)
    }

    /// One of the pane's tabs: plain words, the open one on a soft light fill that glides.
    private func tab(_ title: String, _ value: String) -> some View {
        let on = side == value
        return Button { withMotion(.smooth(duration: 0.3)) { side = value } } label: {
            Text(title)
                .font(.needle(.subheadline).weight(.semibold))
                .foregroundStyle(.white.opacity(on ? 1 : 0.65))
                .padding(.horizontal, 14).padding(.vertical, 7)
                .background {
                    if on { Capsule().fill(.white.opacity(0.16)).matchedGeometryEffect(id: "tab", in: tabs) }
                }
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

extension ColorSchemeContrast {
    /// A dim white sign over the cover's colours: nearly full with Increase Contrast.
    func sign(_ opacity: Double) -> Double {
        self == .increased ? max(opacity, 0.9) : opacity
    }

    /// Grey words over the cover's colours: brighter with Increase Contrast.
    var quiet: AnyShapeStyle {
        self == .increased ? AnyShapeStyle(HierarchicalShapeStyle.primary.opacity(0.85)) : AnyShapeStyle(.secondary)
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
    @Environment(Store.self) private var store
    let symbol: String
    let size: CGFloat
    /// Goes up with each press that should bounce the sign (Shuffle, Repeat).
    var bounce = 0
    let action: () -> Void
    @State private var held = false
    /// The player on a short screen (a phone on its side): a little smaller.
    @Environment(\.compactPlayer) private var compact

    var body: some View {
        let size = compact ? size * 0.78 : size
        return Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size))
                .contentTransition(.symbolEffect(.replace))
                .pressBounce(bounce)
                .frame(width: max(44, size * 1.9), height: max(44, size * 1.9))
                .glassEffect(held ? .regular.tint((store.coverColor ?? .clear).opacity(0.3)) : .identity, in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(Held(held: $held))
        .hoverEffect(.highlight)
    }

    struct Held: ButtonStyle {
        @Binding var held: Bool
        func makeBody(configuration: Configuration) -> some View {
            configuration.label
                // Goes down a little while held, as every player button (`Pressable`); with
                // Reduce motion only the glass shows.
                .scaleEffect(configuration.isPressed && !Motion.reduced ? Pressable.sign.scale : 1)
                .animation(Pressable.motion(pressed: configuration.isPressed), value: configuration.isPressed)
                .onChange(of: configuration.isPressed) { _, now in
                    withFade(.easeOut(duration: 0.2)) { held = now }
                }
        }
    }
}

/// The seek bar: a strip of glass that grows while it is dragged, with the A-B loop marked.
struct SeekBar: View {
    @Environment(Store.self) private var store
    let duration: Double
    var loop: [Double] = []
    /// The place the finger chose: shown until the core reports it (or 1.5 s pass).
    @State private var dragged: Double?
    /// Where the bar stood when the finger started to drag; nil between drags.
    @State private var dragStart: Double?
    /// The drag goes sideways (true) or not (false), from its start; nil between drags.
    @State private var dragSideways: Bool?
    /// Lets go of `dragged` 1.5 s after a seek, if the core has not reported the place by then.
    @State private var settle: Task<Void, Never>?
    /// The song's loud and quiet shape, once measured.
    @State private var shape: [Float] = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotionSystem
    @Environment(\.colorSchemeContrast) private var contrast

    var body: some View {
        let position = dragged ?? store.position
        let part = duration > 0 ? min(1, max(0, position / duration)) : 0
        VStack(spacing: 6) {
            GeometryReader { geo in
                let style = store.choices.seekStyle
                ZStack(alignment: .leading) {
                    if style != "shape" {
                        // Wave, Line, or Thick (Settings › Appearance › Seek bar).
                        StyledSeekLine(style: style, fraction: part,
                                       moving: store.playback?.playing == true && dragged == nil && !(reduceMotionSystem || store.app.reduceMotion),
                                       lit: store.colors.accent.opacity(dragged == nil ? 0.95 : 1), dim: .white.opacity(0.22))
                    }
                    if style == "shape", shape.isEmpty { Capsule().fill(.white.opacity(0.18)) }
                    if loop.count == 2, duration > 0 {
                        Capsule().fill(Color.accentColor.opacity(0.5))
                            .frame(width: max(4, geo.size.width * (loop[1] - loop[0]) / duration))
                            .offset(x: geo.size.width * loop[0] / duration)
                            .frame(height: shape.isEmpty ? nil : 4)
                    }
                    if style == "shape" {
                        if shape.isEmpty {
                            Capsule().fill(store.colors.accent.opacity(dragged == nil ? 0.9 : 1))
                                .frame(width: max(0, geo.size.width * part))
                        } else {
                            WaveBars(heights: shape, part: part, held: dragged != nil, lit: store.colors.accent)
                        }
                    }
                }
                .frame(height: style != "shape" ? 24 : shape.isEmpty ? (dragged == nil ? 7 : 12) : (dragged == nil ? 24 : 30))
                .frame(maxHeight: .infinity)
                .contentShape(.rect)
                .gesture(seekDrag(width: geo.size.width).exclusively(before: seekTap(width: geo.size.width)))
            }
            .frame(height: store.choices.seekStyle != "shape" ? 28 : shape.isEmpty ? 20 : 32)
            // The core has reported the place the finger chose: the bar follows the song again.
            .onChange(of: store.position) { _, now in
                if dragStart == nil, let dragged, settle != nil, abs(now - dragged) < 1 { letGo() }
            }
            .motion(.spring(response: 0.3, dampingFraction: 0.7), value: dragged == nil)
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
                // The core's format text ("FLAC 24/48", "MP3 320"), or the bare format.
                if let song = store.playback?.current, case let format = song.quality.isEmpty ? song.format : song.quality, !format.isEmpty {
                    Text(format).font(.needle(.caption2).weight(.bold)).padding(.horizontal, 6).padding(.vertical, 2)
                        .overlay(Capsule().stroke(.secondary, lineWidth: 1))
                }
                Spacer()
                Text("-" + clock(max(0, duration - position)))
            }
            .font(.needle(.caption).weight(.semibold).monospacedDigit())
            .foregroundStyle(contrast.quiet)
        }
        .task(id: store.playback?.current?.id) {
            shape = []
            guard let song = store.playback?.current, !song.onComputer else { return }
            while !Task.isCancelled {
                let found = await Waveforms.heights(song.id, store: store)
                if !found.isEmpty {
                    if !Task.isCancelled { withFade(.easeOut(duration: 0.3)) { shape = found } }
                    return
                }
                // A server song's shape is measured once its whole file is here: the plain line
                // until then, asked again every 10 s.
                guard song.fromServer else { return }
                try? await Task.sleep(for: .seconds(10))
            }
        }
    }
}

extension SeekBar {
    /// A drag moves the place by the finger's travel, from where the bar stood, as Control
    /// Center's sliders do: touching the bar never seeks, and a swipe down that starts on it
    /// does not either.
    private func seekDrag(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 4)
            .onChanged { g in
                if dragSideways == nil { dragSideways = abs(g.translation.width) >= abs(g.translation.height) }
                guard dragSideways == true, duration > 0, width > 0 else { return }
                if dragStart == nil {
                    // A new drag: the last one's place stays where it is, under the finger.
                    settle?.cancel()
                    settle = nil
                    dragStart = dragged ?? store.position
                }
                let start = dragStart ?? 0
                dragged = min(duration, max(0, start + Double(g.translation.width / width) * duration))
            }
            .onEnded { _ in
                let wasSideways = dragSideways == true
                dragSideways = nil
                dragStart = nil
                guard wasSideways, let dragged else { return }
                seek(to: dragged)
            }
    }

    /// A tap (with no drag) plays from that place.
    private func seekTap(width: CGFloat) -> some Gesture {
        SpatialTapGesture()
            .onEnded { tap in
                guard duration > 0, width > 0 else { return }
                settle?.cancel()
                let place = duration * min(1, max(0, Double(tap.location.x / width)))
                dragged = place
                seek(to: place)
            }
    }

    /// Seeks, and keeps the chosen place shown until the core reports it, or 1.5 s at most:
    /// the bar never flicks back to the old place for a moment.
    private func seek(to place: Double) {
        store.seek(place)
        settle?.cancel()
        settle = Task { @MainActor in
            try? await Task.sleep(for: .seconds(1.5))
            if !Task.isCancelled, dragStart == nil { letGo() }
        }
    }

    private func letGo() {
        settle?.cancel()
        settle = nil
        dragged = nil
    }
}

/// The phone's own volume, as Control Center sets it, drawn as Apple Music's: a thin line
/// between a quiet and a loud speaker, with no knob.
struct VolumeBar: View {
    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "speaker.fill").font(.needle(.caption))
            SystemVolume().frame(height: 34)
            Image(systemName: "speaker.wave.3.fill").font(.needle(.caption))
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
            Image(systemName: "speaker.fill").font(.needle(.caption))
            LineSlider(value: value ?? Double(store.pc?.volume ?? 1)) { value = $0 } done: {
                guard let value else { return }
                store.setComputerVolume(Float(value))
                // Until the computer's next answer has it.
                Task {
                    try? await Task.sleep(for: .seconds(1.5))
                    if self.value == value { self.value = nil }
                }
            }
            Image(systemName: "speaker.wave.3.fill").font(.needle(.caption))
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
        .motion(.spring(response: 0.3, dampingFraction: 0.7), value: held)
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
                    // A video plays; an animated WebP or GIF shows its frames.
                    if animated, let video { if AnimatedImage.isPicture(video) { AnimatedImage(url: video) } else { LoopingVideo(url: video) } }
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
        .animation(Motion.fade(.easeInOut(duration: 0.5)), value: tone)
        .task(id: "\(song?.id ?? "")|\(animated)") {
            video = nil
            let colors = await PageColors.of(song?.artwork, seed: seed)
            guard !Task.isCancelled else { return }
            tone = colors.page.darkened(to: 0.3)
            #if DEBUG
            // Test builds: -animated-cover <file> plays that file as the moving cover (to check
            // animated pictures without adding files beside anyone's music).
            if animated, let test = UserDefaults.standard.string(forKey: "animated-cover") { video = URL(fileURLWithPath: test); return }
            #endif
            guard animated, let id = song?.id, !(song?.onComputer ?? true) else { return }
            let found = await store.movingCover(id)
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
    /// White on the player; the page's ink on the floating bar.
    var tint: UIColor = .white

    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = tint
        view.activeTintColor = UIColor(named: "AccentColor")
        view.prioritizesVideoDevices = false
        return view
    }

    func updateUIView(_ view: AVRoutePickerView, context: Context) {
        view.tintColor = tint
    }
}

/// Timed lyrics that follow the song (tap a line to go there), with each word lit as it is
/// sung when the lyrics have word timing; or the plain words.
struct LyricsPane: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    let song: Song?
    /// The player's Lyrics look: only the words, very large, centred.
    var stage = false
    /// Told whether the song has any lyrics, once they are looked up.
    var found: ((Bool) -> Void)? = nil
    /// On the player's pane of glass (a wide window): its buttons are plain.
    var onPane = false
    @State private var lyrics: Lyrics?
    @State private var loaded = false
    /// Online lookups are on (LRCLIB and the lyrics plugins).
    @State private var online = true
    /// A line chosen to share as a picture.
    @State private var card: CardLine?
    /// Each line in your language, under it, translated on the iPhone (Apple's Translation).
    @AppStorage("translateLyrics") private var translate = false
    @State private var translations: [Int: String] = [:]
    @State private var translation: TranslationSession.Configuration?

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
                            .font(.needle(.footnote)).foregroundStyle(.secondary).multilineTextAlignment(.center)
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
                        .buttonStyle(onPane ? AnyPrimitiveButtonStyle(.bordered) : AnyPrimitiveButtonStyle(.glass))
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
                        .buttonStyle(onPane ? AnyPrimitiveButtonStyle(.bordered) : AnyPrimitiveButtonStyle(.glass))
                        Button("Add and time them") { window.sheet = .timing(song) }
                            .buttonStyle(onPane ? AnyPrimitiveButtonStyle(.bordered) : AnyPrimitiveButtonStyle(.glass))
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
        .onChange(of: translate) { _, _ in askTranslation() }
        .translationTask(translation) { session in
            let lines = lyrics?.lines ?? []
            let requests = lines.enumerated().compactMap { i, line in
                line.text.trimmingCharacters(in: .whitespaces).isEmpty ? nil : TranslationSession.Request(sourceText: line.text, clientIdentifier: String(i))
            }
            do {
                var found: [Int: String] = [:]
                for response in try await session.translations(from: requests) {
                    // A line already in your language is left alone.
                    if let i = response.clientIdentifier.flatMap(Int.init), response.targetText != response.sourceText {
                        found[i] = response.targetText
                    }
                }
                translations = found
                if found.isEmpty { store.message = "These lyrics are already in your language" }
            } catch {
                store.message = "These lyrics can't be translated on this \(Device.name)"
            }
        }
        .sheet(item: $card) { chosen in
            if let song { LyricCardSheet(line: chosen.text, song: song) }
        }
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
        self.found?(!(found?.lines.isEmpty ?? true) || !(found?.plain.isEmpty ?? true))
        askTranslation()
    }

    /// Asks Apple's Translation for these lyrics, into your language, while Translate is on.
    private func askTranslation() {
        translations = [:]
        guard translate, let lines = lyrics?.lines, !lines.isEmpty else { translation = nil; return }
        if translation == nil { translation = TranslationSession.Configuration() } else { translation?.invalidate() }
    }

    private func plain(_ text: String) -> some View {
        ScrollView {
            Text(text).font(stage ? .scaled(30, relativeTo: .title, weight: .heavy) : .needle(.title3).weight(.semibold))
                .multilineTextAlignment(stage ? .center : .leading)
                .frame(maxWidth: .infinity, alignment: stage ? .center : .leading)
                .padding(.vertical, 20)
        }
        .mask(LyricsFade())
    }

    private func timed(_ lines: [LyricLine]) -> some View {
        TimedLyrics(lines: lines, stage: stage, translations: translate ? translations : [:]) { card = CardLine(text: $0) }
    }
}

/// The lyrics fade out at the top and the foot as they scroll.
private struct LyricsFade: View {
    var body: some View {
        LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.08), .init(color: .black, location: 0.8), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom)
    }
}

/// Timed lyrics: the line being sung lit, and scrolled to. Apart from the pane, so only this
/// follows the time; the line being sung lights its words from a clock of its own (see
/// `KaraokeLine`).
private struct TimedLyrics: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    /// Increase Contrast: the other lines brighter and sharp.
    @Environment(\.colorSchemeContrast) private var contrast
    let lines: [LyricLine]
    let stage: Bool
    /// Each line in your language, by its place (Translate on).
    let translations: [Int: String]
    let share: (String) -> Void
    /// When the finger last scrolled the lyrics: the sung line is not scrolled to for 3 seconds
    /// after, so the reader is never pulled back while reading ahead or back.
    @State private var userScrolled: Date?

    /// How long the lyrics wait after a scroll by hand, as on every system.
    private static let handScrollPause: Duration = .seconds(3)

    var body: some View {
        let reduce = systemReduceMotion || store.app.reduceMotion
        let position = store.position
        let current = lines.lastIndex { $0.time <= position + 0.2 } ?? -1
        let anchor = UnitPoint(x: stage ? 0.5 : 0, y: stage ? 0.4 : 0.25)
        return ScrollViewReader { reader in
            ScrollView(showsIndicators: false) {
                LazyVStack(alignment: stage ? .center : .leading, spacing: stage ? 30 : 22) {
                    Color.clear.frame(height: 40)
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        VStack(alignment: stage ? .center : .leading, spacing: 4) {
                            Group {
                                if i == current && !line.words.isEmpty {
                                    KaraokeLine(line: line)
                                } else {
                                    Text(line.text.isEmpty ? "♪" : line.text)
                                }
                            }
                            .font(.scaled(stage ? 38 : 28, relativeTo: .title, weight: .heavy))
                            .multilineTextAlignment(stage ? .center : .leading)
                            // The line in your language, under it (Translate on).
                            if let translated = translations[i] {
                                Text(translated).font(.scaled(stage ? 24 : 19, relativeTo: .title3, weight: .semibold)).opacity(0.62)
                                    .multilineTextAlignment(stage ? .center : .leading)
                            }
                        }
                        .opacity(i == current ? 1 : contrast == .increased ? 0.6 : 0.32)
                        .blur(radius: i == current || reduce || contrast == .increased ? 0 : 1)
                        // Room for the playing line to grow without going past the edge.
                        .padding(.horizontal, stage ? 8 : 0)
                        .padding(.trailing, stage ? 0 : 16)
                        .scaleEffect(i == current && !reduce ? 1.03 : 1, anchor: stage ? .center : .leading)
                        .frame(maxWidth: .infinity, alignment: stage ? .center : .leading)
                        .id(i)
                        .onTapGesture {
                            // A tapped line plays: the lyrics follow it again at once.
                            userScrolled = nil
                            store.seek(line.time)
                        }
                        .contextMenu {
                            if !line.text.trimmingCharacters(in: .whitespaces).isEmpty {
                                Button("Share as Card", systemImage: "square.and.arrow.up") { share(line.text) }
                            }
                        }
                    }
                    Color.clear.frame(height: 200)
                }
                .animation(reduce ? .easeInOut(duration: 0.2) : .spring(response: 0.5, dampingFraction: 0.8), value: current)
            }
            .onScrollPhaseChange { old, phase in
                // The finger on the lyrics, their glide after it, and the moment it ends.
                func byHand(_ p: ScrollPhase) -> Bool { p == .tracking || p == .interacting || p == .decelerating }
                if byHand(phase) || byHand(old) { userScrolled = .now }
            }
            .onChange(of: current, initial: true) { _, now in
                guard now >= 0, userScrolled == nil else { return }
                withMotion(.spring(response: 0.6, dampingFraction: 0.9)) { reader.scrollTo(now, anchor: anchor) }
            }
            // 3 seconds after the last scroll by hand, back to the line being sung.
            .task(id: userScrolled) {
                guard userScrolled != nil else { return }
                try? await Task.sleep(for: Self.handScrollPause)
                guard !Task.isCancelled else { return }
                userScrolled = nil
                let now = lines.lastIndex { $0.time <= store.position + 0.2 } ?? -1
                guard now >= 0 else { return }
                withMotion(.spring(response: 0.6, dampingFraction: 0.9)) { reader.scrollTo(now, anchor: anchor) }
            }
        }
        .mask(LyricsFade())
    }
}

/// The line being sung, its words lit one by one as they are sung. The core says where the
/// song is four times a second; between, the place is worked out from the clock each frame, so
/// the words light on time and one at a time, not late and in bursts.
private struct KaraokeLine: View {
    @Environment(Store.self) private var store
    let line: LyricLine

    var body: some View {
        let clock = store.clock
        // Read here so the line starts again from the core's place at each of its reports.
        let _ = clock.position
        TimelineView(.animation(minimumInterval: 1 / 30, paused: clock.rate == 0)) { context in
            let position = clock.estimate(at: context.date)
            line.words.reduce(Text("")) { text, word in
                Text("\(text)\(Text(word.text).foregroundStyle(word.time <= position ? Color.white : Color.white.opacity(0.4)))")
            }
        }
    }
}

/// The songs up next: tap one to play it, swipe to take it out, drag to move it. On the side
/// pane, Shuffle and Add songs above them, as on the phone's sheet.
struct QueuePane: View {
    @Environment(Store.self) private var store
    /// On the player's pane of glass (a wide window), or on a glass sheet.
    var onPane = false
    /// "Continue Playing" and the album above the songs.
    var titled = true
    /// Shuffle and Add songs above the songs.
    var addSongs: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let addSongs { UpNextActions(addSongs: addSongs) }
            if titled {
                VStack(alignment: .leading, spacing: 2) {
                    Text(store.onComputer ? "Up Next on Your Computer" : "Continue Playing").font(.needle(.headline))
                    if let album = store.playback?.current?.album, !album.isEmpty {
                        Text("From \(album)").font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.6)).lineLimit(1)
                    }
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
                    // Each row keeps its own id (the song, and which time it is in the list), so a
                    // row taken out or moved is that row, never the one after it.
                    ForEach(Array(Store.rowIDs(store.upNext).enumerated()), id: \.element.id) { i, row in
                        let song = row.song
                        Button { store.jump(i) } label: {
                            HStack(spacing: 14) {
                                Artwork(path: song.artwork, seed: song.album + song.artist, size: 150)
                                    .frame(width: 50, height: 50).clipShape(.soft(Corner.cover))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(song.title).lineLimit(1)
                                    Text(song.artist).font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
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

/// A wide pill of Up Next's (shuffle, repeat), as Apple Music's: glass in the cover's colour,
/// light while on.
struct Pill: View {
    @Environment(Store.self) private var store
    let symbol: String
    let on: Bool
    var bounce = 0
    /// On a pane of glass: a plain fill, never glass on glass.
    var plain = false

    var body: some View {
        Image(systemName: symbol)
            .font(.needle(.body).weight(.semibold))
            .pressBounce(bounce)
            .foregroundStyle(on ? Color.black.opacity(0.8) : .white)
            .frame(maxWidth: .infinity)
            .frame(height: plain ? 36 : 40)
            .background { if plain { Capsule().fill(.white.opacity(on ? 0.85 : 0.12)) } }
            .glassEffect(plain ? .identity : on ? .regular.tint(.white.opacity(0.8)) : .regular.tint((store.coverColor ?? .clear).opacity(0.3)), in: .capsule)
            .contentShape(.capsule)
            .contentTransition(.symbolEffect(.replace))
            .hoverEffect(.highlight)
    }
}

/// A player's sign without a frame, as Apple Music's: white, a soft circle behind it while on.
struct PlainSign: View {
    let symbol: String
    let on: Bool

    var body: some View {
        Image(systemName: symbol)
            .font(.needle(.body).weight(.semibold))
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
    /// The speakers of the group playing now, with their delays, moved while it plays.
    @State private var members: [GroupMember] = []

    var body: some View {
        Form {
            Section {
                Toggle(isOn: $phone) { Label("This \(Device.name)", systemImage: Device.symbol) }
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
            // As on the Mac: a group playing now is timed while it plays.
            if !members.isEmpty {
                Section {
                    ForEach(members, id: \.key) { member in
                        VStack(alignment: .leading) {
                            LabeledContent(member.key.isEmpty ? "This \(Device.name)" : member.name) {
                                Text("\(member.delay > 0 ? "+" : "")\(member.delay) ms").monospacedDigit()
                            }
                            Slider(value: Binding(get: { Double(member.delay) }, set: { setDelay(member.key, Int32(($0 / 10).rounded() * 10)) }),
                                   in: -1000...1000, step: 10)
                                .accessibilityLabel(member.key.isEmpty ? "This \(Device.name)" : member.name)
                                .accessibilityValue("\(member.delay) ms")
                        }
                    }
                } header: {
                    Text("Speaker timing")
                } footer: {
                    Text("Move a late or early speaker's slider.")
                }
            }
            let playing = Array(chosen) + (phone ? [""] : [])
            // Lining up a group before it plays (one playing now is timed above).
            if playing.count > 1 && members.isEmpty {
                Section {
                    ForEach(playing, id: \.self) { id in
                        VStack(alignment: .leading) {
                            LabeledContent(id.isEmpty ? "This \(Device.name)" : all?.first { $0.id == id }?.name ?? "Speaker", value: "\(delays[id] ?? 0) ms")
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
        // Half height first, on the system's glass, as the other tools of the player.
        .presentationDetents([.medium, .large])
        .toolSheet(store)
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
            members = await store.load { $0.groupMembers() } ?? []
        }
    }

    /// Moves one speaker of the group playing now earlier or later, at once.
    private func setDelay(_ key: String, _ ms: Int32) {
        guard let i = members.firstIndex(where: { $0.key == key }), members[i].delay != ms else { return }
        members[i].delay = ms
        store.run { try $0.setSpeakerDelay(key: key, ms: ms) }
    }

    private func apply() {
        let ids = Array(chosen)
        let group = ids.count + (phone ? 1 : 0) > 1
        let here = phone || ids.isEmpty
        let timed = !members.isEmpty
        let lined = delays
        store.run { core in
            // A group timed while it played keeps its timing (read again, as it is now).
            let wait = group ? (timed ? core.speakers().delays : lined) : [:]
            try core.setSpeakers(ids: ids, thisPhone: here, delays: wait)
        }
        store.message = ids.isEmpty ? "Playing on this \(Device.name)" : group ? "Playing on \(ids.count + (phone ? 1 : 0)) at once" : "Playing on \(all?.first { $0.id == ids[0] }?.name ?? "the speaker")"
        dismiss()
    }
}

/// Time the lyrics: tap as each line (or word) starts while the song plays; −0.2 s and +0.2 s
/// move the line last tapped, Redo line taps it again. The steps are the core's (`LyricTimer`).
struct TimingView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let song: Song
    @State private var timer: LyricTimer?
    @State private var text = ""
    @State private var loaded = false
    @State private var lines: [TimingLine] = []
    @State private var next = (line: 0, word: 0)
    @State private var selected: Int?
    @State private var words = false
    /// Something changed since it opened.
    @State private var dirty = false
    @State private var taps = 0

    var body: some View {
        let colors = store.colors
        let thisSong = store.playback?.current?.id == song.id
        VStack(spacing: 0) {
            header(thisSong)
            if timer == nil {
                if loaded {
                    List {
                        Section {
                            Text("This song has no lyrics to time. Paste them below, one line per sung line.")
                                .font(.needle(.subheadline)).foregroundStyle(.secondary)
                            TextEditor(text: $text).frame(minHeight: 180)
                            Button("Use these lines") { begin(LyricTimer(lyrics: nil, text: text)) }
                                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    }
                } else {
                    ProgressView().frame(maxHeight: .infinity)
                }
            } else {
                list(thisSong)
                controls(thisSong)
            }
        }
        .navigationTitle("Time the lyrics")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button { dismiss() } label: { Image(systemName: "xmark") }.accessibilityLabel("Close")
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") { save() }.disabled(!dirty).tint(colors.accent)
            }
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Toggle("Time Each Word", systemImage: "textformat.abc", isOn: Binding(get: { words }, set: { on in
                        timer?.setWordMode(on: on)
                        changed()
                    }))
                    .disabled(timer == nil)
                    Button("Forget My Timing", systemImage: "trash", role: .destructive) {
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
            if let found, !found.lines.isEmpty || !found.plain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                begin(LyricTimer(lyrics: found, text: ""), dirty: false)
            }
            loaded = true
        }
    }

    /// The song, where it is, and how far along.
    private func header(_ thisSong: Bool) -> some View {
        let colors = store.colors
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                    .frame(width: 48, height: 48).clipShape(.soft(Corner.cover))
                VStack(alignment: .leading, spacing: 2) {
                    Text(song.title).font(.needle(.headline)).lineLimit(1)
                    Text(song.artist).font(.needle(.subheadline)).foregroundStyle(colors.muted).lineLimit(1)
                }
                Spacer()
                TimingClock(duration: song.duration, playing: thisSong, progress: false)
            }
            if thisSong {
                TimingClock(duration: song.duration, playing: true, progress: true)
            } else {
                Button("Play it from the start", systemImage: "play.fill") { store.play([song]) }
                    .buttonStyle(.glass)
            }
        }
        .padding(.horizontal, 20).padding(.vertical, 12)
    }

    private func list(_ thisSong: Bool) -> some View {
        let colors = store.colors
        return ScrollViewReader { reader in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        let isNext = i == next.line
                        Button { choose(i, at: line.time, thisSong: thisSong) } label: {
                            HStack(alignment: .firstTextBaseline, spacing: 12) {
                                Text(line.time.map(stamp) ?? "--:--.--")
                                    .font(.scaled(12, relativeTo: .caption).monospaced())
                                    .foregroundStyle(selected == i ? ThemeColors.amber : colors.muted)
                                    .frame(width: 66, alignment: .leading)
                                Group {
                                    if words && !line.words.isEmpty {
                                        Text(wordText(line, next: isNext ? next.word : nil))
                                    } else {
                                        Text(line.text.isEmpty ? "♪" : line.text)
                                            .foregroundStyle(line.time != nil || isNext ? colors.text : colors.text.opacity(0.4))
                                    }
                                }
                                .font(.needle(.title3).weight(.semibold))
                                .multilineTextAlignment(.leading)
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 12).padding(.vertical, 8)
                            .background(isNext ? colors.accent.opacity(0.14) : .clear, in: .soft(Corner.small))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .id(i)
                    }
                }
                .padding(.horizontal, 8)
            }
            .onChange(of: next.line) { _, line in withMotion(.smooth) { reader.scrollTo(line, anchor: .center) } }
        }
    }

    /// Word mode: tapped words full strength, the rest faint, the next one underlined.
    private func wordText(_ line: TimingLine, next: Int?) -> AttributedString {
        let colors = store.colors
        var text = AttributedString()
        for (w, word) in line.words.enumerated() {
            var part = AttributedString(word.text)
            if w == next {
                part.foregroundColor = colors.accent
                part.underlineStyle = .single
            } else {
                part.foregroundColor = word.time != nil ? colors.text : colors.text.opacity(0.4)
            }
            text += part
        }
        return text
    }

    /// −0.2 s, Redo line, +0.2 s, and Undo; then the big tap.
    private func controls(_ thisSong: Bool) -> some View {
        let colors = store.colors
        let done = next.line >= lines.count
        return GlassEffectContainer(spacing: 8) {
            VStack(spacing: 12) {
                HStack(spacing: 8) {
                    fix("− 0.2 s", label: "The line starts earlier") { timer?.earlier() }
                    fix("↺ Line", label: "Tap this line again") {
                        timer?.redoLine()
                        if let i = selected, i < lines.count, let time = lines[i].time, thisSong { store.seek(max(0, time - 2)) }
                    }
                    fix("+ 0.2 s", label: "The line starts later") { timer?.later() }
                    Button { if timer?.undo() == true { changed() } } label: {
                        Image(systemName: "arrow.uturn.backward").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glass)
                    .accessibilityLabel("Undo")
                }
                .font(.needle(.subheadline).weight(.semibold))
                Button { tap() } label: {
                    Text(done ? "All timed" : words ? "Tap as the next word starts" : "Tap as the next line starts")
                        .font(.needle(.headline))
                        .frame(maxWidth: .infinity).padding(.vertical, 14)
                }
                .buttonStyle(.glassProminent)
                .foregroundStyle(colors.onAccent)
                .disabled(!thisSong || done)
                .sensoryFeedback(.impact(weight: .light), trigger: taps) { _, _ in store.app.haptics }
            }
        }
        .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 12)
    }

    private func fix(_ title: String, label: String, _ action: @escaping () -> Void) -> some View {
        Button { action(); changed() } label: { Text(title).frame(maxWidth: .infinity).padding(.vertical, 6) }
            .buttonStyle(.glass)
            .disabled(selected == nil)
            .accessibilityLabel(label)
    }

    private func stamp(_ t: Double) -> String {
        String(format: "%02d:%05.2f", Int(t) / 60, t.truncatingRemainder(dividingBy: 60))
    }

    private func begin(_ new: LyricTimer, dirty: Bool = true) {
        timer = new
        self.dirty = dirty
        refresh()
    }

    private func refresh() {
        guard let timer else { return }
        lines = timer.lines()
        next = (Int(timer.nextLine()), Int(timer.nextWord()))
        selected = timer.selected().map(Int.init)
        words = timer.wordMode()
    }

    private func changed() {
        dirty = true
        refresh()
    }

    private func tap() {
        guard let timer, store.playback?.current?.id == song.id else { return }
        timer.tap(position: store.position)
        taps += 1
        changed()
    }

    /// Tapping starts again from this line; a tapped line plays from a little before.
    private func choose(_ i: Int, at time: Double?, thisSong: Bool) {
        timer?.select(index: UInt32(i))
        if let time, thisSong { store.seek(max(0, time - 2)) }
        refresh()
    }

    private func save() {
        guard let timer else { return }
        let id = song.id
        Task {
            guard let saved = await store.ask({ try $0.saveLyricTimer(id: id, timer: timer) }) else { return }
            if saved {
                store.lyricsChanged()
                store.message = "Saved the timed lyrics"
                dismiss()
            } else {
                store.message = "Tap at least one line before saving."
            }
        }
    }
}

/// Where the song is, in the lyrics timer: the time, or a line of how far along. Apart, so
/// only it follows the time.
private struct TimingClock: View {
    @Environment(Store.self) private var store
    let duration: Double
    /// The song timed is the one playing.
    let playing: Bool
    let progress: Bool

    var body: some View {
        let position = playing ? store.position : 0
        if progress {
            ProgressView(value: min(position, duration), total: max(duration, 1)).tint(store.colors.accent)
        } else {
            Text("\(clock(position)) / \(clock(duration))")
                .font(.scaled(13, relativeTo: .footnote).monospaced()).foregroundStyle(store.colors.muted)
        }
    }
}

/// The big player's seek bar as a wave, a line, or a thick bar (Settings › Appearance › Seek
/// bar), as on Android and Windows: what is still to play is a straight bar; what has played is
/// the same in the lit colour, or, for Wave, a line that waves while the music plays (flat while
/// it rests, and with Reduce motion).
struct StyledSeekLine: View {
    /// "wave", "line", or "thick".
    let style: String
    /// Where the song is, 0 to 1.
    let fraction: Double
    /// The wave moves: the music plays and motion is allowed.
    let moving: Bool
    let lit: Color
    let dim: Color

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: !(moving && style == "wave"))) { context in
            Canvas { g, size in
                let thick: CGFloat = style == "thick" ? 10 : style == "wave" ? 3 : 4
                let middle = size.height / 2
                let split = size.width * min(max(fraction, 0), 1)
                // What is still to play.
                let rest = max(0, split - thick / 2)
                g.fill(Path(roundedRect: CGRect(x: rest, y: middle - thick / 2, width: size.width - rest, height: thick), cornerRadius: thick / 2),
                       with: .color(dim))
                guard split > 0 else { return }
                if style == "wave" {
                    // About 34 points a wave, 3.5 high, moving at 6 radians a second.
                    let amplitude: CGFloat = moving ? 3.5 : 0
                    let phase = moving ? (context.date.timeIntervalSinceReferenceDate * 6).truncatingRemainder(dividingBy: 2 * .pi) : 0
                    var line = Path()
                    var x: CGFloat = 0
                    while x <= split {
                        let point = CGPoint(x: x, y: middle + amplitude * CGFloat(sin(Double(x) / 34 * 2 * .pi - phase)))
                        if x == 0 { line.move(to: point) } else { line.addLine(to: point) }
                        x += 1
                    }
                    g.stroke(line, with: .color(lit), style: StrokeStyle(lineWidth: thick, lineCap: .round, lineJoin: .round))
                } else {
                    g.fill(Path(roundedRect: CGRect(x: 0, y: middle - thick / 2, width: max(split, thick), height: thick), cornerRadius: thick / 2),
                           with: .color(lit))
                }
            }
        }
        .accessibilityHidden(true)
    }
}

/// Counts the hinge's changes (opened, half-open, closed) on a folding iPhone, iOS 27.1 on.
struct HingeWatch: ViewModifier {
    @Binding var changes: Int

    func body(content: Content) -> some View {
        if #available(iOS 27.1, *) {
            content.onHingeChange { old, new in
                if old.hinge?.status != new.hinge?.status { changes += 1 }
            }
        } else {
            content
        }
    }
}

extension EnvironmentValues {
    /// The player's song and controls on a short screen (a phone on its side), drawn smaller.
    @Entry var compactPlayer = false
}
