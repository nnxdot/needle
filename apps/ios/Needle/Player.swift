import AVKit
import MediaPlayer
import SwiftUI
import Translation

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
                Text(song?.title ?? "Not Playing").font(.needle(.subheadline).weight(.semibold)).lineLimit(1)
                if placement != .inline, let song {
                    if let sung {
                        // Fades in at each new line; a faint ♪ while none is sung.
                        Text(sung.text.isEmpty ? "♪" : sung.text)
                            .font(.needle(.caption).weight(.medium))
                            .foregroundStyle(sung.text.isEmpty ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.primary))
                            .lineLimit(1)
                            .id(sung.index)
                            .transition(.opacity)
                    } else {
                        Text(store.onComputer ? "On your computer" : song.artist).font(.needle(.caption)).foregroundStyle(.secondary).lineLimit(1)
                    }
                }
            }
            .animation(.easeOut(duration: 0.35), value: sung?.index)
            .frame(maxWidth: .infinity, alignment: .leading)
            Button { store.toggle() } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.needle(.title3))
                    .contentTransition(.symbolEffect(.replace))
                    .frame(width: 36, height: 36)
            }
            .disabled(song == nil)
            .sensoryFeedback(.impact(weight: .light), trigger: playing) { _, _ in store.app.haptics }
            if placement != .inline {
                Button { store.next() } label: {
                    Image(systemName: "forward.fill").font(.needle(.title3)).frame(width: 36, height: 36)
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
    /// Translate in the lyrics (the lyrics pane reads the same switch).
    @AppStorage("translateLyrics") private var translateLyrics = false
    /// The visualizer over the whole screen.
    @State private var showVisualizer = false
    /// The song has lyrics (for the Lyrics look; with none, it shows the cover).
    @State private var hasLyrics = true

    /// The look chosen with the Look button: the cover, a turning record, only the lyrics, or
    /// the visualizer.
    private var look: String {
        let chosen = store.choices.bigLook
        if chosen == "lyrics" && !hasLyrics { return "cover" }
        return ["cover", "record", "lyrics", "visualizer"].contains(chosen) ? chosen : "cover"
    }
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
    /// Counts the hinge's changes (iPhone Duo), so the parts are placed again.
    @State private var hingeChanges = 0
    /// Wider than tall (a phone on its side, the iPhone Duo open).
    @State private var sideways = false

    enum Pane { case cover, lyrics, queue }

    private var reduceMotion: Bool { systemReduceMotion || store.app.reduceMotion }

    var body: some View {
        let song = store.playback?.current
        let seed = (song?.album ?? "") + (song?.artist ?? "")
        let app = store.app
        let look = look
        // On its side the cover stands at the left, as on Android, never filling the page.
        let fills = app.immersiveCover && look == "cover" && !sideways
        let immersive = fills && pane == .cover
        ZStack {
            Color.black.ignoresSafeArea()
            if fills {
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
            if pane == .cover && (look == "record" || look == "lyrics") {
                // The Record and Lyrics looks stand on the cover's colours, not near black.
                LinearGradient(colors: [(store.coverColor ?? store.colors.accent).opacity(0.5), (store.coverColor ?? store.colors.accent).opacity(0.12)],
                               startPoint: .top, endPoint: .bottom)
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
            }
            VStack(spacing: 0) {
                top(song)
                // The iPhone Duo open: the look and the song's controls side by side, parted
                // along the screen's fold (or above and below it, held as a tent); closed, or
                // on any other phone, one above the other.
                GeometryReader { geo in
                    Group {
                    switch split(geo) {
                    case .phone:
                        VStack(spacing: 0) {
                            stage(song, seed: seed, look: look, immersive: immersive, room: geo.size)
                            panel(song)
                        }
                    case .beside(let left, let gap):
                        // As on Android: on a short screen (a phone on its side) the song and
                        // its controls are a little smaller, and scroll if they still do not fit.
                        let short = geo.size.height < 480
                        HStack(spacing: 0) {
                            VStack(spacing: 0) { stage(song, seed: seed, look: look, immersive: immersive, room: CGSize(width: left, height: geo.size.height)) }
                                .frame(width: left)
                            Color.clear.frame(width: gap)
                            ScrollView(.vertical) {
                                VStack(spacing: 0) { panel(song, compact: short) }
                                    .frame(minHeight: geo.size.height)
                            }
                            .scrollBounceBehavior(.basedOnSize)
                            .scrollIndicators(.hidden)
                            .environment(\.compactPlayer, short)
                        }
                    case .stacked(let above, let gap):
                        VStack(spacing: 0) {
                            VStack(spacing: 0) { stage(song, seed: seed, look: look, immersive: immersive, room: CGSize(width: geo.size.width, height: above)) }
                                .frame(height: above)
                            Color.clear.frame(height: gap)
                            panel(song)
                            Spacer(minLength: 0)
                        }
                    }
                    }
                    .frame(width: geo.size.width, height: geo.size.height)
                }
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 8)
        }
        .modifier(HingeWatch(changes: $hingeChanges))
        .onGeometryChange(for: Bool.self) { $0.size.width > $0.size.height } action: { sideways = $0 }
        .animation(reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.85), value: pane)
        .task(id: song?.id) {
            pendingLoop = nil
            // The new song's star at once, with no empty star between.
            await readFavorite(reset: true)
        }
        .onChange(of: store.playback?.loopStart) { _, _ in pendingLoop = nil }
        // Stars changed elsewhere (the Rate menu): read again, with no blink.
        .task(id: store.ratingVersion) { await readFavorite() }
        .fullScreenCover(isPresented: $showVisualizer) { VisualizerScreen() }
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

    /// How the player parts on a wide or folding screen.
    private enum Split {
        /// One above the other, as on a phone.
        case phone
        /// Side by side: the look this wide, then a gap (the fold, when there is one).
        case beside(CGFloat, CGFloat)
        /// Above and below a fold across the screen: the look this tall, then the fold.
        case stacked(CGFloat, CGFloat)
    }

    /// Where the screen divides (iOS 27.1 tells: the iPhone Duo's fold), else side by side on a
    /// screen wider than tall (a phone on its side), else one above the other.
    private func split(_ geo: GeometryProxy) -> Split {
        _ = hingeChanges
        let size = geo.size
        if #available(iOS 27.1, *),
           let fold = geo.reservedRegions(kind: .division, options: .includeInactive).first?.frame, !fold.isNull, !fold.isEmpty {
            if fold.width >= fold.height, fold.minY > 160, size.height - fold.maxY > 160 { return .stacked(fold.minY, fold.height) }
            if fold.height > fold.width, fold.minX > 200, size.width - fold.maxX > 200 { return .beside(fold.minX, fold.width) }
        }
        // As on Android: side by side when wider than tall and either wide (600 points) or short
        // (under 480, a phone on its side); the song and its controls take 46% (at most 520).
        if size.width > size.height && (size.width >= 600 || size.height < 480) {
            let side = min(size.width * 0.46, 520)
            return .beside(size.width - side - 28, 28)
        }
        return .phone
    }

    /// The look: the cover, the record, the lyrics, or the visualizer (or the lyrics or the
    /// queue pane, under the song small).
    @ViewBuilder
    private func stage(_ song: Song?, seed: String, look: String, immersive: Bool, room: CGSize) -> some View {
        switch pane {
        case .cover:
            if look == "record" {
                // The cover as a turning record, its arm resting on it; still while
                // paused or with Reduce motion.
                Spacer(minLength: 8)
                RecordDisc(side: max(120, min(330, room.width - 16, room.height - 48)), spinning: store.playback?.playing == true && !reduceMotion) {
                    Artwork(path: song?.artwork, seed: seed, size: 500)
                }
                .gesture(swipeToSkip)
                Spacer(minLength: 16)
            } else if look == "lyrics" {
                // Only the words, very large, lit as they are sung.
                LyricsPane(song: song, stage: true) { has in hasLyrics = has }
            } else if look == "visualizer" {
                // The visualizer in the cover's place, on the cover's colours.
                Spacer(minLength: 8)
                Visualizer().frame(maxWidth: 340)
                    .contentShape(.rect)
                    .gesture(swipeToSkip)
                Spacer(minLength: 16)
            } else if immersive {
                // The cover is the page itself: the words sit over its foot.
                Spacer(minLength: 8)
                    .contentShape(.rect)
                    .gesture(swipeToSkip)
            } else {
                Spacer(minLength: 8)
                cover(song, seed: seed)
                Spacer(minLength: 16)
            }
        case .lyrics:
            small(song, seed: seed)
            LyricsPane(song: song)
        case .queue:
            small(song, seed: seed)
            QueuePane()
        }
    }

    /// The song's name (with the cover showing), the seek bar, the controls, and the buttons
    /// at the foot.
    @ViewBuilder
    private func panel(_ song: Song?, compact: Bool = false) -> some View {
        if pane == .cover { title(song, compact: compact) }
        SeekBar(duration: song?.duration ?? 0, loop: loop).padding(.top, compact ? 8 : 16)
        controls.padding(.top, compact ? -8 : 0)
        if store.onComputer {
            ComputerVolume().padding(.top, compact ? 2 : 6)
        } else if store.app.volumeSlider {
            VolumeBar().frame(height: 34).padding(.top, compact ? 2 : 6)
        }
        bottom.padding(.top, compact ? 6 : 14)
    }

    /// The top, as Apple Music's: a grabber to tap or pull down to close the player (and, on
    /// the computer, a word that it plays there).
    private func top(_ song: Song?) -> some View {
        VStack(spacing: 6) {
            Capsule().fill(.white.opacity(0.4)).frame(width: 38, height: 5)
            if store.onComputer {
                Text("Playing on your computer").font(.needle(.caption).weight(.semibold)).foregroundStyle(.white.opacity(0.7))
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: 34)
        .contentShape(.rect)
        .onTapGesture { dismiss() }
        .overlay(alignment: .trailing) { if pane == .cover { lookButton } }
        .gesture(DragGesture(minimumDistance: 10).onEnded { g in
            if g.translation.height > 80 { dismiss() }
        })
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel("Close the player")
        .padding(.top, 4)
    }

    /// The Look button: steps through Cover, Record, Lyrics, and Visualizer (kept in `big_look`).
    private var lookButton: some View {
        let looks = ["cover", "record", "lyrics", "visualizer"]
        let names = ["cover": "Cover", "record": "Record", "lyrics": "Lyrics", "visualizer": "Visualizer"]
        let icons = ["cover": "photo", "record": "record.circle", "lyrics": "quote.bubble", "visualizer": "waveform.circle"]
        let now = store.choices.bigLook
        let next = looks[((looks.firstIndex(of: now) ?? 0) + 1) % looks.count]
        return Button {
            hasLyrics = true
            withAnimation(reduceMotion ? nil : .easeInOut(duration: 0.35)) { store.updateChoices { $0.bigLook = next } }
        } label: {
            Image(systemName: icons[now] ?? "photo")
                .font(.body.weight(.semibold))
                .frame(width: 34, height: 34)
        }
        .buttonStyle(.glass)
        .buttonBorderShape(.circle)
        .accessibilityLabel("Look")
        .accessibilityValue(names[now] ?? "Cover")
        .accessibilityHint("Shows the player as \(names[next] ?? "")")
        .sensoryFeedback(.selection, trigger: now) { _, _ in store.app.haptics }
    }

    /// The song's menu, a plain sign beside its name.
    private func moreMenu(_ song: Song?) -> some View {
        Menu {
            if let song, !song.onComputer {
                SongMenu(song: song, inPlayer: true)
                Divider()
                loopMenu(song)
            }
            if !store.onComputer {
                Button("Speed and Pitch", systemImage: "gauge.with.dots.needle.67percent") { store.sheet = .speed }
            }
            if !store.onComputer {
                Button("Visualizer", systemImage: "waveform.circle") { showVisualizer = true }
            }
            Button("Play on…", systemImage: "hifispeaker.2") { store.sheet = .speakers }
        } label: {
            Image(systemName: "ellipsis").font(.needle(.title3).weight(.semibold))
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
                // The moving cover (Moving covers on) plays in the square or the record too,
                // cropped to it, while the cover pane shows.
                .overlay { MovingCoverLayer(song: song, on: store.app.animatedCovers && !reduceMotion && pane == .cover) }
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

    private func title(_ song: Song?, compact: Bool = false) -> some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "").font(.needle(compact ? .headline : .title3).bold()).lineLimit(1)
                artistButton(song).font(.needle(compact ? .headline : .title3))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !(song?.onComputer ?? false), chosen.contains("favorite") { favoriteButton(song) }
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
            Image(systemName: favorite ? "star.fill" : "star").font(.needle(.title3).weight(.semibold))
                .foregroundStyle(.white)
                .frame(width: 36, height: 36)
                .contentShape(.circle)
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(favorite ? "Unfavorite" : "Favorite")
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
                Text(song?.title ?? "").font(.needle(.title3).weight(.semibold)).lineLimit(1)
                artistButton(song).font(.needle(.body))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !(song?.onComputer ?? false), chosen.contains("favorite") { favoriteButton(song) }
            moreMenu(song)
        }
        .padding(.vertical, 14)
        .transition(.opacity)
    }

    /// The player's buttons chosen in Settings › Appearance › Player buttons.
    private var chosen: [String] { PlayerButtons.chosen(store.choices.playerButtons) }

    private var controls: some View {
        HStack {
            Spacer()
            ForEach(chosen.filter { PlayerButtons.transport.contains($0) }, id: \.self) { name in
                transportButton(name)
                Spacer()
            }
        }
        .padding(.top, 10)
    }

    @ViewBuilder
    private func transportButton(_ name: String) -> some View {
        let playing = store.playback?.playing == true
        let jump = store.app.sideButtons == "jump"
        let repeatMode = store.playback?.repeat ?? .off
        switch name {
        case "previous":
            if jump { BigButton(symbol: "gobackward.10", size: 32) { store.jumpBy(-10) }.accessibilityLabel("Back 10 seconds") }
            else { BigButton(symbol: "backward.fill", size: 34) { store.previous() }.accessibilityLabel("Previous") }
        case "next":
            if jump { BigButton(symbol: "goforward.10", size: 32) { store.jumpBy(10) }.accessibilityLabel("On 10 seconds") }
            else { BigButton(symbol: "forward.fill", size: 34) { store.next() }.accessibilityLabel("Next") }
        case "shuffle":
            BigButton(symbol: "shuffle", size: 22) { store.shuffle() }
                .opacity(0.8)
                .accessibilityLabel("Shuffle up next")
        case "repeat":
            BigButton(symbol: repeatMode == .one ? "repeat.1" : "repeat", size: 22) { store.cycleRepeat() }
                .opacity(repeatMode == .off ? 0.55 : 1)
                .accessibilityLabel("Repeat")
                .accessibilityValue(repeatMode == .off ? "Off" : repeatMode == .all ? "All" : "One")
        default:
            BigButton(symbol: playing ? "pause.fill" : "play.fill", size: 46) { store.toggle() }
                .sensoryFeedback(.impact(weight: .medium), trigger: playing) { _, _ in store.app.haptics }
                .accessibilityLabel(playing ? "Pause" : "Play")
        }
    }

    /// Lyrics, speakers, and the queue: plain signs, as Apple Music's, the one open lit; with
    /// the lyrics open, Translate beside them.
    private var bottom: some View {
        let foot = chosen.filter { PlayerButtons.foot.contains($0) }
        return HStack {
            ForEach(Array(foot.enumerated()), id: \.element) { i, name in
                if i > 0 { Spacer() }
                footButton(name)
            }
            if foot.isEmpty { Spacer() }
            // The sleep timer's moon, on this iPhone's own music only.
            if !store.onComputer {
                Spacer()
                SleepMoon()
            }
        }
        .padding(.horizontal, 40)
    }

    @ViewBuilder
    private func footButton(_ name: String) -> some View {
        switch name {
        case "lyrics":
            HStack {
                paneButton(.lyrics, symbol: "quote.bubble")
                if pane == .lyrics { translateButton }
            }
        case "queue": paneButton(.queue, symbol: "list.bullet")
        default: RoutePicker().frame(width: 44, height: 44).opacity(0.75)
        }
    }

    /// Beside the lyrics while they are open: each line in your language under it.
    private var translateButton: some View {
        HStack {
            if pane == .lyrics {
                Button { translateLyrics.toggle() } label: {
                    Image(systemName: "translate")
                        .font(.needle(.title3).weight(translateLyrics ? .semibold : .regular))
                        .foregroundStyle(.white.opacity(translateLyrics ? 1 : 0.7))
                        .frame(width: 44, height: 44)
                        .background(.white.opacity(translateLyrics ? 0.2 : 0), in: .circle)
                        .contentShape(.circle)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(translateLyrics ? "Hide the translation" : "Translate the lyrics")
                .accessibilityAddTraits(translateLyrics ? .isSelected : [])
            }
        }
    }

    private func paneButton(_ which: Pane, symbol: String) -> some View {
        let on = pane == which
        return Button { pane = on ? .cover : which } label: {
            Image(systemName: on && which == .lyrics ? "quote.bubble.fill" : symbol)
                .font(.needle(.title3).weight(on ? .semibold : .regular))
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
    /// The player on a short screen (a phone on its side): a little smaller.
    @Environment(\.compactPlayer) private var compact

    var body: some View {
        let size = compact ? size * 0.78 : size
        return Button(action: action) {
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
    /// The song's loud and quiet shape, once measured.
    @State private var shape: [Float] = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotionSystem

    var body: some View {
        let position = dragged ?? store.playback?.position ?? 0
        let part = duration > 0 ? min(1, max(0, position / duration)) : 0
        VStack(spacing: 6) {
            GeometryReader { geo in
                let style = store.choices.seekStyle
                ZStack(alignment: .leading) {
                    if style != "shape" {
                        // Wave, Line, or Thick (Settings › Appearance › Seek bar).
                        StyledSeekLine(style: style, fraction: part,
                                       moving: store.playback?.playing == true && dragged == nil && !(reduceMotionSystem || store.app.reduceMotion),
                                       lit: .white.opacity(dragged == nil ? 0.85 : 1), dim: .white.opacity(0.22))
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
                            Capsule().fill(.white.opacity(dragged == nil ? 0.75 : 0.95))
                                .frame(width: max(0, geo.size.width * part))
                        } else {
                            WaveBars(heights: shape, part: part, held: dragged != nil)
                        }
                    }
                }
                .frame(height: style != "shape" ? 24 : shape.isEmpty ? (dragged == nil ? 7 : 12) : (dragged == nil ? 24 : 30))
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
            .frame(height: store.choices.seekStyle != "shape" ? 28 : shape.isEmpty ? 20 : 32)
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
                    Text(format).font(.needle(.caption2).weight(.bold)).padding(.horizontal, 6).padding(.vertical, 2)
                        .overlay(Capsule().stroke(.secondary, lineWidth: 1))
                }
                Spacer()
                Text("-" + clock(max(0, duration - position)))
            }
            .font(.needle(.caption).weight(.semibold).monospacedDigit())
            .foregroundStyle(.secondary)
        }
        .task(id: store.playback?.current?.id) {
            shape = []
            guard let song = store.playback?.current, !song.onComputer else { return }
            while !Task.isCancelled {
                let found = await Waveforms.heights(song.id, store: store)
                if !found.isEmpty {
                    if !Task.isCancelled { withAnimation(.easeOut(duration: 0.3)) { shape = found } }
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
        .animation(.easeInOut(duration: 0.5), value: tone)
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
    /// The player's Lyrics look: only the words, very large, centred.
    var stage = false
    /// Told whether the song has any lyrics, once they are looked up.
    var found: ((Bool) -> Void)? = nil
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
                store.message = "These lyrics can't be translated on this iPhone"
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
            Text(text).font(stage ? .system(size: 30, weight: .heavy) : .needle(.title3).weight(.semibold))
                .multilineTextAlignment(stage ? .center : .leading)
                .frame(maxWidth: .infinity, alignment: stage ? .center : .leading)
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
                LazyVStack(alignment: stage ? .center : .leading, spacing: stage ? 30 : 22) {
                    Color.clear.frame(height: 40)
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        VStack(alignment: stage ? .center : .leading, spacing: 4) {
                            Group {
                                if i == current && !line.words.isEmpty {
                                    karaoke(line, at: position)
                                } else {
                                    Text(line.text.isEmpty ? "♪" : line.text)
                                }
                            }
                            .font(.system(size: stage ? 38 : 28, weight: .heavy))
                            .multilineTextAlignment(stage ? .center : .leading)
                            // The line in your language, under it (Translate on).
                            if translate, let translated = translations[i] {
                                Text(translated).font(.system(size: stage ? 24 : 19, weight: .semibold)).opacity(0.62)
                                    .multilineTextAlignment(stage ? .center : .leading)
                            }
                        }
                        .opacity(i == current ? 1 : 0.32)
                        .blur(radius: i == current ? 0 : 1)
                        // Room for the playing line to grow without going past the edge.
                        .padding(.horizontal, stage ? 8 : 0)
                        .padding(.trailing, stage ? 0 : 16)
                        .scaleEffect(i == current ? 1.03 : 1, anchor: stage ? .center : .leading)
                        .frame(maxWidth: .infinity, alignment: stage ? .center : .leading)
                        .id(i)
                        .onTapGesture { store.seek(line.time) }
                        .contextMenu {
                            if !line.text.trimmingCharacters(in: .whitespaces).isEmpty {
                                Button("Share as Card", systemImage: "square.and.arrow.up") { card = CardLine(text: line.text) }
                            }
                        }
                    }
                    Color.clear.frame(height: 200)
                }
                .animation(.spring(response: 0.5, dampingFraction: 0.8), value: current)
            }
            .onChange(of: current, initial: true) { _, now in
                guard now >= 0 else { return }
                withAnimation(.spring(response: 0.6, dampingFraction: 0.9)) { reader.scrollTo(now, anchor: UnitPoint(x: stage ? 0.5 : 0, y: stage ? 0.4 : 0.25)) }
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
                Text(store.onComputer ? "Up Next on Your Computer" : "Continue Playing").font(.needle(.headline))
                if let album = store.playback?.current?.album, !album.isEmpty {
                    Text("From \(album)").font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.6)).lineLimit(1)
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

/// A wide pill of Up Next's (shuffle, repeat), as Apple Music's: soft, light while on.
struct Pill: View {
    let symbol: String
    let on: Bool

    var body: some View {
        Image(systemName: symbol)
            .font(.needle(.body).weight(.semibold))
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
        let position = thisSong ? store.playback?.position ?? 0 : 0
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                    .frame(width: 48, height: 48).clipShape(.rect(cornerRadius: 8))
                VStack(alignment: .leading, spacing: 2) {
                    Text(song.title).font(.needle(.headline)).lineLimit(1)
                    Text(song.artist).font(.needle(.subheadline)).foregroundStyle(colors.muted).lineLimit(1)
                }
                Spacer()
                Text("\(clock(position)) / \(clock(song.duration))")
                    .font(.system(size: 13, design: .monospaced)).foregroundStyle(colors.muted)
            }
            if thisSong {
                ProgressView(value: min(position, song.duration), total: max(song.duration, 1)).tint(colors.accent)
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
                                    .font(.system(size: 12, design: .monospaced))
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
                            .background(isNext ? colors.accent.opacity(0.14) : .clear, in: RoundedRectangle(cornerRadius: 10))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .id(i)
                    }
                }
                .padding(.horizontal, 8)
            }
            .onChange(of: next.line) { _, line in withAnimation { reader.scrollTo(line, anchor: .center) } }
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
        return VStack(spacing: 12) {
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
        timer.tap(position: store.playback?.position ?? 0)
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
