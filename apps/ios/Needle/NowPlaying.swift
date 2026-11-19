import AVFoundation
import SwiftUI

/// The full player, as the user chose it (2026-11-18, `now-playing/mix.html`): the cover far out
/// of focus behind all under a veil of its own colour; the immersive cover sharp at the top
/// going out of focus below (or the cover as a plain card); two round glass buttons at the top;
/// the title with the heart, the lyric line, the seek bar, the main buttons, and one glass pill
/// of tools. On an iPad the layout comes from the shape of the player's area (the window less
/// the side pane): taller than wide, the phone's layout; wider, side by side.
struct NowPlayingView: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    /// Increase Contrast: the player's dim signs and grey words brighter.
    @Environment(\.colorSchemeContrast) private var contrast
    @Environment(\.wide) private var wide
    /// On a phone: the cover, or the lyrics in its place with the song small above them.
    @State private var pane = Pane.cover
    /// Translate in the lyrics (the lyrics read the same switch).
    @AppStorage("translateLyrics") private var translateLyrics = false
    /// The visualizer over the whole screen.
    @State private var showVisualizer = false
    /// The song has lyrics (for the Lyrics look; with none, it shows the cover).
    @State private var hasLyrics = true
    @State private var favorite = false
    @State private var favoriteTaps = 0
    /// A loop just set, shown until the core plays it (a moment later).
    @State private var pendingLoop: [Double]?
    /// How far the cover is pulled sideways (or thrown, after a skip).
    @State private var swipe: CGFloat = 0
    /// The pull on the cover goes sideways (true) or not (false), from its start; nil between.
    @State private var swipeSideways: Bool?
    /// Counts the covers thrown, so only the last throw brings the next cover in.
    @State private var coverThrows = 0
    /// The player's size: how far a thrown cover goes, and how far a pull closes.
    @State private var playerSize = CGSize(width: 400, height: 800)
    /// How far the top pulls the player down (with Reduce motion, where the zoom is off).
    @State private var pull: CGFloat = 0
    /// Counts the hinge's changes (iPhone Duo), so the parts are placed again.
    @State private var hingeChanges = 0
    /// The glass sheets of the player: More, Up next (a phone's), and Add songs.
    @State private var showMore = false
    @State private var showQueue = false
    @State private var showAddSongs = false
    /// What a sheet's row opens once that sheet has gone (one sheet at a time).
    @State private var afterSheet: (() -> Void)?
    /// The pane beside the player in a wide window: "lyrics", "queue", or "" while it is closed.
    /// Kept between times the player opens; Up next the first time.
    @AppStorage(NowPlayingView.sidePaneKey) private var side = "queue"
    /// The status bar's height, for the immersive cover that runs under it.
    @State private var safeTop: CGFloat = 0

    enum Pane { case cover, lyrics }

    /// The pane beside the player, kept between times the player opens (the floating bar's
    /// Lyrics and Up Next set it too).
    static let sidePaneKey = "player-side-pane"

    private var reduceMotion: Bool { systemReduceMotion || store.app.reduceMotion }

    /// The A-B loop the core plays: it ends it itself (another song, the song chosen again).
    private var loop: [Double] {
        if let start = store.playback?.loopStart, let end = store.playback?.loopEnd { return [start, end] }
        return pendingLoop ?? []
    }

    /// The look chosen in More: the cover, a turning record, the lyrics, or the visualizer.
    private var look: String {
        let chosen = store.choices.bigLook
        if chosen == "lyrics" && !hasLyrics { return "cover" }
        return ["cover", "record", "lyrics", "visualizer"].contains(chosen) ? chosen : "cover"
    }

    /// The side pane can show: a wide window (an iPad's).
    private var paneOpen: Bool { wide && (side == "lyrics" || side == "queue") }

    /// The lyrics show in the cover's place (a phone's Lyrics, or the Lyrics look).
    private var lyricsInPlace: Bool { !paneOpen && (pane == .lyrics || look == "lyrics") }

    /// The buttons chosen in Settings › Appearance › Player buttons.
    private var chosen: [String] { PlayerButtons.chosen(store.choices.playerButtons) }

    var body: some View {
        let song = store.playback?.current
        let seed = (song?.album ?? "") + (song?.artist ?? "")
        ZStack {
            PlayerBackdrop(song: song, seed: seed)
            GeometryReader { geo in
                let full = geo.size
                let paneWidth: CGFloat = paneOpen ? min(380, max(300, full.width * 0.34)) : 0
                let area = CGSize(width: full.width - (paneOpen ? paneWidth + 12 : 0), height: full.height)
                // As on Android after the fold test: the area's own shape picks the layout.
                let beside = area.height > 0 && area.width >= area.height * 0.9
                HStack(spacing: 0) {
                    Group {
                        if beside {
                            besideLayout(song, seed: seed, area: area)
                        } else {
                            phoneLayout(song, seed: seed, area: area)
                        }
                    }
                    .frame(width: area.width, height: area.height)
                    .transition(.opacity)
                    .id(beside)
                    if paneOpen {
                        Color.clear.frame(width: 12)
                        PlayerSidePane(side: $side, song: song, addSongs: { showAddSongs = true })
                            .frame(width: paneWidth)
                            .padding(.bottom, 8)
                            .transition(Motion.transition(.move(edge: .trailing).combined(with: .opacity)))
                    }
                }
                .padding(.trailing, paneOpen ? 12 : 0)
                .animation(Motion.fade(.easeInOut(duration: 0.3)), value: beside)
                .onChange(of: beside, initial: true) { _, now in pillShown = !now }
            }
            // How far below the screen's top the player's parts start (the immersive cover
            // runs up under the status bar from there).
            .onGeometryChange(for: CGFloat.self) { $0.frame(in: .global).minY } action: { safeTop = $0 }
            // The round buttons and the pane's top under the status bar, where the safe area
            // leaves the bar over the page (an iPad on its side).
            .modifier(ClearsStatusBar())
        }
        .modifier(HingeWatch(changes: $hingeChanges))
        .onGeometryChange(for: CGSize.self) { $0.size } action: { playerSize = $0 }
        .task(id: song?.id) {
            pendingLoop = nil
            hasLyrics = true
            // The new song's heart at once, with no empty heart between.
            await readFavorite(reset: true)
        }
        .onChange(of: store.playback?.loopStart) { _, _ in pendingLoop = nil }
        // Stars changed elsewhere: read again, with no blink.
        .task(id: store.ratingVersion) { await readFavorite() }
        .fullScreenCover(isPresented: $showVisualizer) { VisualizerScreen() }
        .sheet(isPresented: $showMore, onDismiss: runAfterSheet) {
            MoreSheet(song: song, loop: loop, pillShown: pillShown, setLoop: setLoop, stopLoop: stopLoop,
                      setLook: setLook, fullScreen: { showVisualizer = true }, after: { work in afterSheet = work; showMore = false })
        }
        .sheet(isPresented: $showQueue, onDismiss: runAfterSheet) {
            UpNextSheet(addSongs: { afterSheet = { showAddSongs = true }; showQueue = false })
        }
        .sheet(isPresented: $showAddSongs) { AddSongsSheet() }
        .onChange(of: song == nil) { _, gone in if gone { dismiss() } }
        // The screen stays on while the player shows (Keep the screen on, under The player), or
        // while the lyrics show (the same switch under Lyrics).
        .onChange(of: store.app.keepScreenOnPlayer || ((paneOpen ? side == "lyrics" : lyricsInPlace) && store.app.keepScreenOnLyrics), initial: true) { _, on in
            UIApplication.shared.isIdleTimerDisabled = on
        }
        .grain(store.app.grain)
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
        // Asked from the floating bar or the keyboard: the lyrics or the queue.
        .onChange(of: window.playerPane, initial: true) { _, asked in
            guard let asked else { return }
            window.playerPane = nil
            if wide { side = asked } else if asked == "lyrics" { pane = .lyrics } else { showQueue = true }
        }
        #if DEBUG
        .task {
            let args = CommandLine.arguments
            if args.contains("-show-lyrics") { pane = .lyrics; side = "lyrics" }
            if args.contains("-pane-closed") { side = "" }
            if args.contains("-pane-queue") { side = "queue" }
            // A sheet a moment after the player is up (one presented with it never shows).
            try? await Task.sleep(for: .seconds(1.5))
            if args.contains("-show-queue") { if wide { side = "queue" } else { showQueue = true } }
            if args.contains("-show-more") { showMore = true }
        }
        #endif
        // The player is always dark: its page is the cover's own dark colours, whatever the
        // theme (light text and glass on them, and a light status bar), and its signs white.
        .tint(.white)
        .preferredColorScheme(.dark)
        .offset(y: pull)
    }

    /// The tools pill shows in the phone's layout; side by side it does not (its tools are then
    /// round buttons at the top, and Sleep timer and Play on are in More).
    @State private var pillShown = true

    private func runAfterSheet() {
        let work = afterSheet
        afterSheet = nil
        work?()
    }

    // MARK: Layouts

    /// Taller than wide: the cover across the top (or a card), then the song and its controls.
    private func phoneLayout(_ song: Song?, seed: String, area: CGSize) -> some View {
        let immersive = store.app.immersiveCover && look == "cover" && !lyricsInPlace
        // The cover from the very top (under the status bar) to just above the song's name.
        let coverHeight = min(area.width * 1.22, (area.height + safeTop) * 0.62)
        return VStack(spacing: 0) {
            topBar(song, roundTools: false)
            Group {
                if lyricsInPlace {
                    lyricsInCoverPlace(song, seed: seed)
                } else {
                    stage(song, seed: seed, immersive: immersive)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            panel(song, compact: false, centred: true, withPill: true)
                .frame(maxWidth: 560)
        }
        .frame(maxWidth: .infinity)
        .background(alignment: .topLeading) {
            if immersive {
                // On an iPad beside the pane the cover goes on under it and fades there.
                SharpCover(song: song, seed: seed, animated: store.app.animatedCovers && !reduceMotion, fadeRight: paneOpen ? 116 : 0)
                    .frame(width: area.width + (paneOpen ? 108 : 0), height: coverHeight)
                    .offset(x: swipe, y: -safeTop)
                    .allowsHitTesting(false)
            }
        }
    }

    /// Wider than tall: the cover at the left (filling it from top to foot, or as a card), the
    /// song and its controls at the right.
    private func besideLayout(_ song: Song?, seed: String, area: CGSize) -> some View {
        let short = area.height < 480
        let immersive = store.app.immersiveCover && look == "cover" && !lyricsInPlace
        let tall = area.height + safeTop
        let coverWidth = min(max(tall, area.width * 0.40), area.width * 0.58)
        let leftWidth = immersive ? coverWidth : min(area.width * 0.5, max(area.width * 0.42, min(tall * 0.62, area.width * 0.46) + 48))
        return ZStack(alignment: .top) {
            HStack(spacing: 0) {
                ZStack {
                    if lyricsInPlace {
                        VStack(spacing: 0) {
                            Color.clear.frame(height: 50)
                            lyricsInCoverPlace(song, seed: seed)
                        }
                        .padding(.leading, 24)
                    } else if immersive {
                        Color.clear
                            .contentShape(.rect)
                            .gesture(store.app.swipeCover ? skipDrag : nil)
                    } else {
                        let side = max(120, min(tall * 0.62, area.width * 0.46, area.height - 40))
                        besideLook(song, seed: seed, side: side)
                    }
                }
                .frame(width: leftWidth)
                ScrollView(.vertical) {
                    VStack(spacing: 0) {
                        Spacer(minLength: short ? 50 : 60)
                        panel(song, compact: short, centred: false, withPill: false)
                            .frame(maxWidth: 560)
                        Spacer(minLength: short ? 8 : 24)
                    }
                    .frame(minHeight: area.height)
                    .padding(.horizontal, 28)
                }
                .scrollBounceBehavior(.basedOnSize)
                .scrollIndicators(.hidden)
                .environment(\.compactPlayer, short)
            }
            topBar(song, roundTools: true)
            if !immersive && !lyricsInPlace && !store.onComputer {
                SpeakerPill()
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .padding(.trailing, 24).padding(.bottom, 16)
            }
        }
        .background(alignment: .topLeading) {
            if immersive {
                SharpCover(song: song, seed: seed, animated: store.app.animatedCovers && !reduceMotion, fadeRight: coverWidth * 0.32, fadeBottom: false)
                    .frame(width: coverWidth, height: tall)
                    .offset(x: swipe, y: -safeTop)
                    .allowsHitTesting(false)
            }
        }
    }

    /// The look at the left of a side-by-side layout, `side` square.
    @ViewBuilder
    private func besideLook(_ song: Song?, seed: String, side: CGFloat) -> some View {
        switch look {
        case "record":
            RecordDisc(side: side, spinning: store.playback?.playing == true && !reduceMotion) { Artwork(path: song?.artwork, seed: seed, size: 700) }
                .offset(x: swipe)
                .gesture(store.app.swipeCover ? skipDrag : nil)
        case "visualizer":
            VisualizerBars().frame(width: side, height: side * 0.7)
                .contentShape(.rect)
                .offset(x: swipe)
                .gesture(store.app.swipeCover ? skipDrag : nil)
        default:
            cover(song, seed: seed).frame(width: side, height: side)
        }
    }

    /// The look in a phone's layout: the room above the song (the immersive cover is drawn
    /// behind, so here there is only room to swipe), a card, the record, or the visualizer.
    @ViewBuilder
    private func stage(_ song: Song?, seed: String, immersive: Bool) -> some View {
        if immersive {
            Color.clear
                .contentShape(.rect)
                .gesture(store.app.swipeCover ? skipDrag : nil)
                .accessibilityHidden(true)
        } else {
            GeometryReader { g in
                // Sized from the room the area leaves, never a fixed size: as large as fits.
                let side = max(100, min(g.size.width - 52, g.size.height - 24, 620))
                Group {
                    switch look {
                    case "record":
                        RecordDisc(side: side, spinning: store.playback?.playing == true && !reduceMotion) { Artwork(path: song?.artwork, seed: seed, size: 700) }
                            .offset(x: swipe)
                            .gesture(store.app.swipeCover ? skipDrag : nil)
                    case "visualizer":
                        VisualizerBars().frame(width: side, height: side * 0.7)
                            .contentShape(.rect)
                            .offset(x: swipe)
                            .gesture(store.app.swipeCover ? skipDrag : nil)
                    default:
                        cover(song, seed: seed).frame(width: side, height: side)
                    }
                }
                .frame(width: g.size.width, height: g.size.height)
            }
        }
    }

    /// The lyrics in the cover's place: the song small at the top, the words under it.
    private func lyricsInCoverPlace(_ song: Song?, seed: String) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 14) {
                Artwork(path: song?.artwork, seed: seed, size: 240)
                    .frame(width: 56, height: 56)
                    .clipShape(.soft(10))
                    .shadow(color: .black.opacity(0.3), radius: 9, y: 4)
                VStack(alignment: .leading, spacing: 1) {
                    Text(song?.title ?? "").font(.needle(.title3).bold()).lineLimit(1)
                    artistButton(song).font(.needle(.subheadline))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                translateButton
                if !(song?.onComputer ?? false), chosen.contains("favorite") { heart(song) }
            }
            .padding(.horizontal, 26)
            .padding(.top, 8)
            LyricsPane(song: song) { has in if look == "lyrics" { hasLyrics = has } }
                .padding(.horizontal, 26)
        }
        .frame(maxWidth: 620)
        .transition(.opacity)
    }

    /// The song, the lyric line, the seek bar, the main buttons, and (on a phone's layout) the
    /// pill of tools.
    @ViewBuilder
    private func panel(_ song: Song?, compact: Bool, centred: Bool, withPill: Bool) -> some View {
        let wideTitle = !centred && !compact && wide
        VStack(spacing: 0) {
            if !lyricsInPlace {
                title(song, size: wideTitle ? 44 : compact ? 24 : 30)
                PlayerLyricLine(song: song, centred: centred) { pane = .lyrics; if paneOpen || (wide && !centred) { side = "lyrics" } }
            }
            SeekBar(duration: song?.duration ?? 0, loop: loop).padding(.top, compact ? 8 : 14)
            MainButtons(chosen: chosen.filter { PlayerButtons.transport.contains($0) }, scale: compact ? 0.78 : 1)
                .padding(.top, compact ? 4 : 10)
            if store.onComputer {
                ComputerVolume().padding(.top, compact ? 2 : 8)
            } else if store.app.volumeSlider {
                VolumeBar().frame(height: 34).padding(.top, compact ? 2 : 8)
            }
            if withPill {
                ToolsPill(lyricsOn: paneOpen ? side == "lyrics" : lyricsInPlace,
                          queueOn: paneOpen ? side == "queue" : showQueue,
                          moreOn: showMore,
                          lyrics: toggleLyrics, queue: toggleQueue, more: { showMore = true })
                    .padding(.top, compact ? 12 : 20)
                    .padding(.bottom, 10)
            }
        }
        .padding(.horizontal, centred ? 26 : 0)
    }

    /// The title, bold and large (it never slides and never loses its start: a long one ends in
    /// "…"), the artist under it, and the heart at its right.
    private func title(_ song: Song?, size: CGFloat) -> some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "")
                    .font(.scaled(size, relativeTo: .title, weight: .bold))
                    .kerning(-0.3)
                    .lineLimit(1)
                    .truncationMode(.tail)
                artistButton(song).font(.scaled(size * 0.55, relativeTo: .body))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if !(song?.onComputer ?? false), chosen.contains("favorite") { heart(song) }
        }
    }

    /// The top: close at the left, More at the right, round glass on the picture. Side by side
    /// (no pill), Lyrics and Up next are round buttons there too; the Visualizer look adds Full
    /// screen.
    private func topBar(_ song: Song?, roundTools: Bool) -> some View {
        HStack(spacing: 10) {
            RoundGlass(symbol: "chevron.down", label: "Close the player") { dismiss() }
            Spacer(minLength: 8)
            if look == "visualizer" && !store.onComputer {
                RoundGlass(symbol: "arrow.up.left.and.arrow.down.right", label: "Full screen") { showVisualizer = true }
            }
            if roundTools {
                RoundGlass(symbol: "quote.bubble", label: "Lyrics", on: paneOpen ? side == "lyrics" : lyricsInPlace, action: toggleLyrics)
                RoundGlass(symbol: "list.bullet", label: "Up Next", on: paneOpen ? side == "queue" : showQueue, action: toggleQueue)
            }
            RoundGlass(symbol: "ellipsis", label: "More", on: showMore) { showMore = true }
                // In a wide window Speed and Pitch, Play on and the sleep timer open as popovers.
                .toolPopover(roundTools ? ["speed", "speakers", "sleep"] : ["speed"])
        }
        .padding(.horizontal, 18)
        .frame(height: 50)
        .contentShape(.rect)
        // The zoom the player opens with closes by a pull down itself. Without it (Reduce
        // motion) the top pulls the player down.
        .gesture(reduceMotion ? pullToClose : nil)
    }

    private func toggleLyrics() {
        if wide {
            withMotion(.smooth(duration: 0.3)) { side = side == "lyrics" ? "" : "lyrics" }
        } else if lyricsInPlace {
            withMotion(.spring(response: 0.45, dampingFraction: 0.85)) {
                pane = .cover
                if look == "lyrics" { store.updateChoices { $0.bigLook = "cover" } }
            }
        } else {
            withMotion(.spring(response: 0.45, dampingFraction: 0.85)) { pane = .lyrics }
        }
    }

    private func toggleQueue() {
        if wide {
            withMotion(.smooth(duration: 0.3)) { side = side == "queue" ? "" : "queue" }
        } else {
            showQueue = true
        }
    }

    private func setLook(_ next: String) {
        hasLyrics = true
        withFade(.easeInOut(duration: 0.35)) {
            store.updateChoices { $0.bigLook = next }
            if next != "lyrics" { pane = .cover }
        }
    }

    // MARK: Parts

    private func cover(_ song: Song?, seed: String) -> some View {
        let playing = store.playback?.playing == true
        return Artwork(path: song?.artwork, seed: seed, size: 1000)
            .aspectRatio(1, contentMode: .fit)
            // The moving cover (Moving covers on) plays in the card too, cropped to it.
            .overlay { MovingCoverLayer(song: song, on: store.app.animatedCovers && !reduceMotion) }
            .clipShape(.soft(Corner.player))
            // A plain card: a soft deep shadow, no glass round it.
            .shadow(color: .black.opacity(0.4), radius: 27, y: 26)
            .shadow(color: .black.opacity(0.26), radius: 6, y: 4)
            // Smaller while paused, as Apple Music's; with Reduce motion it stays its size.
            .scaleEffect(playing || reduceMotion ? 1 : 0.9)
            .offset(x: swipe)
            .animation(reduceMotion ? nil : .spring(response: 0.55, dampingFraction: 0.8), value: playing)
            .gesture(store.app.swipeCover ? skipDrag : nil)
            .accessibilityHidden(true)
    }

    /// The artist's name, which opens the artist's page.
    @ViewBuilder
    private func artistButton(_ song: Song?) -> some View {
        if let song, !song.onComputer, !isPlaceholderArtist(song.artist) {
            Button { window.opening = .artist(song.artist) } label: {
                Text(song.artist).foregroundStyle(.white.opacity(contrast.sign(0.72))).lineLimit(1)
            }
            .buttonStyle(.plain)
            .accessibilityHint("Opens the artist")
        } else {
            Text(store.onComputer ? "On your computer" : song?.artist ?? "").foregroundStyle(.white.opacity(contrast.sign(0.72))).lineLimit(1)
        }
    }

    private func heart(_ song: Song?) -> some View {
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
            Image(systemName: favorite ? "heart.fill" : "heart")
                .font(.system(size: 25, weight: .semibold))
                .foregroundStyle(store.colors.accent)
                .frame(width: 44, height: 44)
                .contentShape(.circle)
                .contentTransition(.symbolEffect(.replace))
                .pressBounce(favoriteTaps)
        }
        .buttonStyle(Pressable.sign)
        .padding(.trailing, -8)
        .accessibilityLabel(favorite ? "Remove from Favorites" : "Add to Favorites")
        .sensoryFeedback(.success, trigger: favoriteTaps) { _, _ in store.app.haptics }
    }

    /// Beside the song over the lyrics: each line in your language under it.
    private var translateButton: some View {
        Button {
            withFade(.smooth(duration: 0.3)) { translateLyrics.toggle() }
        } label: {
            Image(systemName: "translate")
                .font(.needle(.title3).weight(translateLyrics ? .semibold : .regular))
                .foregroundStyle(translateLyrics ? store.colors.accent : .white.opacity(contrast.sign(0.7)))
                .frame(width: 44, height: 44)
                .contentShape(.circle)
        }
        .buttonStyle(Pressable.sign)
        .accessibilityLabel(translateLyrics ? "Hide the translation" : "Translate the lyrics")
        .accessibilityAddTraits(translateLyrics ? .isSelected : [])
    }

    // MARK: Gestures and state

    /// The top's pull: the player follows the finger and closes after a quarter of its height
    /// or a flick down.
    private var pullToClose: some Gesture {
        DragGesture(minimumDistance: 10, coordinateSpace: .global)
            .onChanged { g in
                let down = g.translation.height
                pull = down >= 0 ? down : Motion.rubberBand(down, limit: 60)
            }
            .onEnded { g in
                let down = g.translation.height, speed = g.velocity.height
                if (down > playerSize.height * 0.25 && speed > -800) || speed > 900 {
                    dismiss()
                } else {
                    withAnimation(Motion.thrown(speed, over: -pull, duration: 0.3)) { pull = 0 }
                }
            }
    }

    /// Sideways on the cover: it follows the finger, and a long enough pull or a flick plays the
    /// next or the last song.
    private var skipDrag: some Gesture {
        DragGesture(minimumDistance: 20, coordinateSpace: .global)
            .onChanged { g in
                if swipeSideways == nil { swipeSideways = abs(g.translation.width) > abs(g.translation.height) }
                guard swipeSideways == true else { return }
                swipe = g.translation.width
            }
            .onEnded { g in
                let wasSideways = swipeSideways == true
                swipeSideways = nil
                guard wasSideways else { return }
                let pulled = g.translation.width, speed = g.velocity.width
                // As on every system: over 28% of the cover's width, or a flick of 800 points a
                // second, unless the finger flicks back the other way.
                let far = min(playerSize.width, 520) * 0.28
                let ahead = speed < -800 || (pulled < -far && speed < 800)
                let back = speed > 800 || (pulled > far && speed > -800)
                if ahead || back {
                    throwCover(ahead: ahead, speed: speed)
                } else {
                    withAnimation(Motion.thrown(speed, over: -swipe, duration: 0.4, bounce: 0.1)) { swipe = 0 }
                }
            }
    }

    /// The cover leaves the way it was thrown; then the song changes and the next cover comes in
    /// from the other side.
    private func throwCover(ahead: Bool, speed: CGFloat) {
        let skip = { if ahead { store.next() } else { store.previous() } }
        guard !Motion.reduced else {
            skip()
            withoutMotion { swipe = 0 }
            return
        }
        coverThrows += 1
        let run = coverThrows
        let gone = (ahead ? -1 : 1) * max(playerSize.width, 320)
        withAnimation(Motion.thrown(speed, over: gone - swipe, duration: 0.3), completionCriteria: .logicallyComplete) {
            swipe = gone
        } completion: {
            skip()
            guard run == coverThrows, swipeSideways == nil else { return }
            withoutMotion { swipe = -gone * 0.1 }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(16))
                guard run == coverThrows, swipeSideways == nil else { return }
                withMotion(.spring(duration: 0.35, bounce: 0)) { swipe = 0 }
            }
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

    /// The core takes the loop a moment later: More and the seek bar show it at once.
    private func setLoop(_ start: Double, _ end: Double) {
        guard end > start else { return }
        store.core.setLoop(start: start, end: end)
        pendingLoop = [start, end]
    }

    private func stopLoop() {
        store.core.setLoop(start: nil, end: nil)
        pendingLoop = nil
    }
}

// MARK: - The backdrop and the covers

/// Behind all: the cover far out of focus under a veil of its own colour, a little darker at
/// the edges. With Reduce Transparency (or the blurred cover off), the cover's colour flat.
struct PlayerBackdrop: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    let song: Song?
    let seed: String
    @State private var tone: Color

    init(song: Song?, seed: String) {
        self.song = song
        self.seed = seed
        // The colour already known, so it never starts from black.
        _tone = State(initialValue: PageColors.cached(song?.artwork, seed: seed)?.page.darkened(to: 0.32) ?? .black)
    }

    var body: some View {
        ZStack {
            tone
            if store.app.coverBackdrop && !reduceTransparency {
                CoverBackdrop(path: song?.artwork, seed: seed, blur: CGFloat(store.app.backdropBlur),
                              moving: store.app.movingBackdrop && !(systemReduceMotion || store.app.reduceMotion))
                    .brightness(-0.08)
                // The veil: the cover's own colour over it, deeper toward the foot.
                LinearGradient(colors: [tone.opacity(0.34), tone.opacity(0.66)], startPoint: .top, endPoint: .bottom)
            }
            // A little darker at the edges.
            RadialGradient(stops: [.init(color: .clear, location: 0.3), .init(color: .black.opacity(0.38), location: 1)],
                           center: UnitPoint(x: 0.5, y: 0.36), startRadius: 0, endRadius: 700)
        }
        .ignoresSafeArea()
        .allowsHitTesting(false)
        .animation(Motion.fade(.easeInOut(duration: 0.5)), value: tone)
        .task(id: song?.id) {
            let colors = await PageColors.of(song?.artwork, seed: seed)
            if !Task.isCancelled { tone = colors.page.darkened(to: 0.32) }
        }
    }
}

/// The immersive cover: sharp at the top, edge to edge, going out of focus into the backdrop
/// with no hard edge (and, beside a pane or side by side, fading out at its right too).
struct SharpCover: View {
    let song: Song?
    let seed: String
    let animated: Bool
    /// Points at the right that fade out (none: a hard edge at the window's side).
    var fadeRight: CGFloat = 0
    /// The foot fades out (the phone's layout); side by side it runs to the foot.
    var fadeBottom = true

    var body: some View {
        GeometryReader { geo in
            Artwork(path: song?.artwork, seed: seed, size: 1400)
                .overlay { MovingCoverLayer(song: song, on: animated) }
                .aspectRatio(1, contentMode: .fill)
                .frame(width: geo.size.width, height: geo.size.height)
                .clipped()
                // The status bar and the round buttons read over a bright cover.
                .overlay(alignment: .top) {
                    LinearGradient(colors: [.black.opacity(0.3), .clear], startPoint: .top, endPoint: .bottom)
                        .frame(height: geo.size.height * 0.22)
                }
                .mask {
                    let right = geo.size.width > 0 ? min(1, fadeRight / geo.size.width) : 0
                    LinearGradient(stops: fadeBottom
                                   ? [.init(color: .black, location: 0), .init(color: .black, location: 0.5), .init(color: .clear, location: 1)]
                                   : [.init(color: .black, location: 0), .init(color: .black, location: 1)],
                                   startPoint: .top, endPoint: .bottom)
                        .mask {
                            LinearGradient(stops: [.init(color: .black, location: 0), .init(color: .black, location: 1 - right), .init(color: .clear, location: 1)],
                                           startPoint: .leading, endPoint: .trailing)
                        }
                }
        }
        .accessibilityHidden(true)
    }
}

/// The Visualizer look: bars in the accent that move with the music; still with Reduce motion.
struct VisualizerBars: View {
    @Environment(Store.self) private var store
    @Environment(\.accessibilityReduceMotion) private var systemReduceMotion
    private static let bands = 28

    var body: some View {
        let accent = store.colors.accent
        TimelineView(.animation(minimumInterval: 1 / 30, paused: systemReduceMotion || store.app.reduceMotion)) { timeline in
            Canvas { g, size in
                _ = timeline.date
                let bands = store.core.spectrum(bands: UInt32(Self.bands))
                guard bands.count == Self.bands else { return }
                let step = size.width / CGFloat(Self.bands)
                for (i, v) in bands.enumerated() {
                    let h = max(6, min(1, CGFloat(v)) * size.height)
                    let rect = CGRect(x: CGFloat(i) * step + step * 0.18, y: (size.height - h) / 2, width: step * 0.64, height: h)
                    g.fill(Path(roundedRect: rect, cornerRadius: step * 0.32), with: .color(accent.opacity(0.55 + 0.45 * Double(min(1, v)))))
                }
            }
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Buttons

/// A round button of glass at the player's top (42), the sign white, or in the accent while on.
struct RoundGlass: View {
    @Environment(Store.self) private var store
    let symbol: String
    let label: String
    var on = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(on ? store.colors.accent : .white)
                .frame(width: 42, height: 42)
                .glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.22)).interactive(), in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(Pressable.sign)
        .hoverEffect(.highlight)
        .accessibilityLabel(label)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The pill of tools: one glass pill in the dock's shape, five icons with no names (Lyrics, Up
/// next, Play on, Sleep timer, More); one that is on or open is in the accent.
struct ToolsPill: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @Environment(\.colorSchemeContrast) private var contrast
    let lyricsOn: Bool
    let queueOn: Bool
    let moreOn: Bool
    let lyrics: () -> Void
    let queue: () -> Void
    let more: () -> Void

    var body: some View {
        let sleeping = store.sleep
        HStack(spacing: 0) {
            tool("quote.bubble", "Lyrics", on: lyricsOn, action: lyrics)
            tool("list.bullet", "Up Next", on: queueOn, action: queue)
            tool("airplayaudio", "Play on", on: store.onComputer) { window.sheet = .speakers }
                .toolPopover(["speakers"])
            if !store.onComputer {
                tool(sleeping == nil ? "moon" : "moon.fill", "Sleep timer", on: sleeping != nil) { window.sheet = .sleep }
                    .accessibilityValue(sleeping.map { "\(sleepClock($0.remaining)) left" } ?? "Off")
                    .toolPopover(["sleep"])
            }
            tool("ellipsis", "More", on: moreOn, action: more)
        }
        .frame(height: 58)
        .glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.22)), in: .capsule)
    }

    private func tool(_ symbol: String, _ label: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 21, weight: on ? .semibold : .regular))
                .foregroundStyle(on ? store.colors.accent : .white.opacity(contrast.sign(0.9)))
                .contentTransition(.symbolEffect(.replace))
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .contentShape(.rect)
        }
        .buttonStyle(Pressable.sign)
        .hoverEffect(.highlight)
        .accessibilityLabel(label)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The card's pill at the foot of a side-by-side player: the speaker in use, and the volume.
private struct SpeakerPill: View {
    @Environment(Store.self) private var store
    @Environment(WindowState.self) private var window
    @State private var speaker = ""

    var body: some View {
        HStack(spacing: 10) {
            Button { window.sheet = .speakers } label: {
                HStack(spacing: 8) {
                    Image(systemName: "airplayaudio").font(.system(size: 17, weight: .semibold))
                    Text(speaker).font(.needle(.subheadline).weight(.medium)).lineLimit(1)
                }
                .contentShape(.rect)
            }
            .buttonStyle(Pressable.sign)
            .toolPopover(["speakers"])
            .accessibilityLabel("Play on")
            .accessibilityValue(speaker)
            if store.app.volumeSlider {
                Rectangle().fill(.white.opacity(0.2)).frame(width: 1, height: 18)
                VolumeBar().frame(width: 170, height: 30)
            }
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 18)
        .frame(height: 46)
        .glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.22)), in: .capsule)
        .task {
            update()
            for await _ in NotificationCenter.default.notifications(named: AVAudioSession.routeChangeNotification) { update() }
        }
    }

    private func update() {
        speaker = AVAudioSession.sharedInstance().currentRoute.outputs.first?.portName ?? Device.name
    }
}

/// Play's six styles (Settings › Appearance › The player › Buttons), saved as on Android in
/// `control_style`: an older "round", "minimal", or "expressive" is Accent, as the first time.
enum PlayStyle: String, CaseIterable {
    case accent, glass, wide, ring, square, trio

    static func of(_ saved: String) -> PlayStyle { PlayStyle(rawValue: saved) ?? .accent }

    var name: String {
        switch self {
        case .accent: "Accent"
        case .glass: "Glass"
        case .wide: "Wide"
        case .ring: "Ring"
        case .square: "Soft square"
        case .trio: "Trio"
        }
    }
}

/// The main buttons in the user's order: plain icons with no circles; Shuffle and Repeat, when
/// on, in the accent with a small dot under them; Play in the chosen style.
struct MainButtons: View {
    @Environment(Store.self) private var store
    @Environment(\.colorSchemeContrast) private var contrast
    let chosen: [String]
    /// The layout's size: 1 on a phone, less on a short one.
    var scale: CGFloat = 1
    @State private var playPresses = 0
    @State private var shufflePresses = 0
    @State private var repeatPresses = 0

    var body: some View {
        let style = PlayStyle.of(store.app.controlStyle)
        let order = chosen
        let i = order.firstIndex(of: "play") ?? 0
        // Trio: Play and the button at each side of it, in one glass pill.
        let trio = style == .trio ? Array(order[max(0, i - 1)...min(order.count - 1, i + 1)]) : []
        HStack(spacing: 0) {
            ForEach(Array(order.enumerated()), id: \.element) { k, name in
                if trio.contains(name) {
                    if name == trio.first {
                        if k > 0 { Spacer(minLength: 0) }
                        trioPill(trio)
                    }
                } else {
                    if k > 0 { Spacer(minLength: 0) }
                    button(name, style: style)
                }
            }
        }
        .padding(.horizontal, order.count <= 3 ? 30 * scale : -4 * scale)
    }

    private func trioPill(_ names: [String]) -> some View {
        HStack(spacing: 4 * scale) {
            ForEach(names, id: \.self) { name in
                if name == "play" { play(.trio) } else { button(name, style: .trio).frame(width: 56 * scale) }
            }
        }
        .padding(.horizontal, 4 * scale)
        .frame(height: 68 * scale)
        .glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.22)), in: .capsule)
    }

    @ViewBuilder
    private func button(_ name: String, style: PlayStyle) -> some View {
        let jump = store.app.sideButtons == "jump"
        let repeatMode = store.playback?.repeat ?? .off
        switch name {
        case "previous":
            if jump { icon("gobackward.10", big: true) { store.jumpBy(-10) }.accessibilityLabel("Back 10 seconds") }
            else { icon("backward.fill", big: true) { store.previous() }.accessibilityLabel("Previous") }
        case "next":
            if jump { icon("goforward.10", big: true) { store.jumpBy(10) }.accessibilityLabel("On 10 seconds") }
            else { icon("forward.fill", big: true) { store.next() }.accessibilityLabel("Next") }
        case "shuffle":
            let stays = store.choices.shuffleStays && !store.onComputer
            let on = stays && store.shuffleOn
            icon("shuffle", big: false, on: on, bounce: shufflePresses) { shufflePresses += 1; store.shuffleButton() }
                .accessibilityLabel(stays ? "Shuffle" : "Shuffle up next")
                .accessibilityValue(stays ? (on ? "On" : "Off") : "")
        case "repeat":
            icon(repeatMode == .one ? "repeat.1" : "repeat", big: false, on: repeatMode != .off, bounce: repeatPresses) { repeatPresses += 1; store.cycleRepeat() }
                .accessibilityLabel("Repeat")
                .accessibilityValue(repeatMode == .off ? "Off" : repeatMode == .all ? "All" : "One")
        default:
            play(style)
        }
    }

    /// A plain icon: 38 for Back and Next, 24 (a little dim) for Shuffle and Repeat.
    private func icon(_ symbol: String, big: Bool, on: Bool = false, bounce: Int = 0, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: (big ? 34 : 22) * scale, weight: big ? .regular : .semibold))
                .foregroundStyle(on ? store.colors.accent : .white.opacity(big ? 1 : contrast.sign(0.66)))
                .contentTransition(.symbolEffect(.replace))
                .pressBounce(bounce)
                .frame(width: 54 * scale, height: 54 * scale)
                .overlay(alignment: .bottom) {
                    if on { Circle().fill(store.colors.accent).frame(width: 4, height: 4).padding(.bottom, 4 * scale) }
                }
                .contentShape(.circle)
        }
        .buttonStyle(Pressable.sign)
        .hoverEffect(.highlight)
    }

    /// Play in the chosen style; only its shape changes, the press and the names stay.
    private func play(_ style: PlayStyle) -> some View {
        let playing = store.playback?.playing == true
        let colors = store.colors
        let side = 76 * scale
        return Button { playPresses += 1; store.toggle() } label: {
            Image(systemName: playing ? "pause.fill" : "play.fill")
                .font(.system(size: (style == .trio ? 26 : 34) * scale, weight: .semibold))
                .contentTransition(.symbolEffect(.replace))
                .foregroundStyle(style == .glass ? Color.white : style == .ring ? colors.accent : colors.onAccent)
                .frame(width: style == .wide ? 116 * scale : style == .trio ? 68 * scale : side,
                       height: style == .wide ? 68 * scale : style == .trio ? 54 * scale : side)
                .background { playShape(style) }
                .contentShape(.rect)
        }
        .buttonStyle(Pressable.sign)
        .hoverEffect(.lift)
        .sensoryFeedback(.impact(weight: .medium), trigger: playPresses) { _, _ in store.app.haptics }
        .accessibilityLabel(playing ? "Pause" : "Play")
    }

    @ViewBuilder
    private func playShape(_ style: PlayStyle) -> some View {
        let accent = store.colors.accent
        switch style {
        case .accent:
            Circle().fill(accent).shadow(color: .black.opacity(0.28), radius: 13, y: 10)
        case .glass:
            Circle().fill(.clear).glassEffect(.regular.tint((store.coverColor ?? .clear).opacity(0.22)).interactive(), in: .circle)
        case .wide, .trio:
            Capsule().fill(accent).shadow(color: .black.opacity(style == .trio ? 0 : 0.28), radius: 13, y: 10)
        case .ring:
            Circle().strokeBorder(accent, lineWidth: 2.5)
        case .square:
            RoundedRectangle(cornerRadius: 26 * scale, style: .continuous).fill(accent).shadow(color: .black.opacity(0.28), radius: 13, y: 10)
        }
    }
}

// MARK: - The lyric line

/// One bold, centred line of the lyrics that moves with the song, as the mini player has it. A
/// press opens the lyrics. A song with no timed lyrics has no line, and the parts close up.
struct PlayerLyricLine: View {
    @Environment(Store.self) private var store
    let song: Song?
    var centred = true
    let open: () -> Void
    @State private var lines: [LyricLine] = []

    var body: some View {
        // A stack, not a Group: with no line a Group has nothing to run the task on.
        VStack(spacing: 0) {
            if !lines.isEmpty {
                Button(action: open) {
                    Line(lines: lines, centred: centred)
                        .frame(maxWidth: .infinity, alignment: centred ? .center : .leading)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .padding(.top, 16)
                .accessibilityHint("Opens the lyrics")
                .transition(.opacity)
            }
        }
        .animation(Motion.fade(), value: lines.isEmpty)
        .task(id: "\(song?.id ?? "")|\(store.lyricsVersion)") {
            lines = []
            guard let song else { return }
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

    /// Apart, so only it follows the time.
    private struct Line: View {
        @Environment(Store.self) private var store
        let lines: [LyricLine]
        let centred: Bool

        var body: some View {
            let position = store.position
            let index = lines.lastIndex(where: { $0.time <= position + 0.2 }) ?? -1
            let text = index >= 0 ? lines[index].text.trimmingCharacters(in: .whitespaces) : ""
            ZStack {
                Text(text.isEmpty ? "♪" : text)
                    .font(.scaled(15.5, relativeTo: .subheadline, weight: .bold))
                    .foregroundStyle(text.isEmpty ? .white.opacity(0.4) : .white)
                    .multilineTextAlignment(centred ? .center : .leading)
                    .lineLimit(1)
                    .id(index)
                    .transition(Motion.transition(AnyTransition(.blurReplace)))
            }
            .frame(maxWidth: .infinity, alignment: centred ? .center : .leading)
            .animation(Motion.fade(.easeOut(duration: 0.2)), value: index)
            .accessibilityLabel(text.isEmpty ? "Lyrics" : text)
        }
    }
}

// MARK: - More

/// More, a glass sheet (drawing A): the song with its stars; four pills (Playlist, Radio, the
/// album, the artist); the player (the look, A-B loop, Speed and pitch, Full screen); then,
/// under a thin line, the rare rows. In a wide window two short columns.
struct MoreSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.wide) private var wide
    @Environment(\.dismiss) private var dismiss
    @Environment(WindowState.self) private var window
    let song: Song?
    let loop: [Double]
    /// The pill of tools is on the screen: Sleep timer and Play on are there, not here.
    let pillShown: Bool
    let setLoop: (Double, Double) -> Void
    let stopLoop: () -> Void
    let setLook: (String) -> Void
    /// The visualizer over the whole screen.
    let fullScreen: () -> Void
    /// Closes this sheet, then does the work (another sheet waits until this one is gone).
    let after: (@escaping () -> Void) -> Void
    @State private var stars = 0
    @State private var height: CGFloat = 520
    /// One press acts once.
    @State private var acted = false

    var body: some View {
        let own = song.map { !$0.onComputer } ?? false
        Group {
            if wide {
                HStack(alignment: .top, spacing: 24) {
                    VStack(alignment: .leading, spacing: 0) {
                        songHead(own: own)
                        if own { pills; Divider().overlay(.white.opacity(0.13)).padding(.vertical, 8); rare }
                    }
                    .frame(maxWidth: .infinity)
                    VStack(alignment: .leading, spacing: 0) { playerPart }
                        .frame(maxWidth: .infinity)
                }
                .padding(.horizontal, 26)
                .padding(.top, 20)
                .padding(.bottom, 20)
                .frame(maxWidth: 720)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        songHead(own: own)
                        if own { pills }
                        playerPart
                        if own {
                            Divider().overlay(.white.opacity(0.13)).padding(.vertical, 6)
                            rare
                        }
                    }
                    .padding(.horizontal, 26)
                    .padding(.top, 22)
                    .padding(.bottom, 12)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
                }
                .scrollBounceBehavior(.basedOnSize)
            }
        }
        .foregroundStyle(.white)
        .tint(store.colors.accent)
        .modifier(MoreSizing(wide: wide, height: height))
        .presentationDragIndicator(.visible)
        .preferredColorScheme(.dark)
        .task(id: "\(song?.id ?? "")|\(store.ratingVersion)") {
            guard let id = song?.id, own else { return }
            // The song's stars as the player has them; a change here is kept at once.
            if store.playback?.current?.id == id { stars = Int(store.playback?.current?.rating ?? 0) }
        }
    }

    /// The cover, the title, and the stars under it.
    private func songHead(own: Bool) -> some View {
        HStack(spacing: 14) {
            Artwork(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), size: 240)
                .frame(width: 56, height: 56)
                .clipShape(.soft(10))
                .shadow(color: .black.opacity(0.3), radius: 9, y: 4)
            VStack(alignment: .leading, spacing: 6) {
                Text(song?.title ?? "").font(.needle(.title3).bold()).lineLimit(1).truncationMode(.tail)
                if own { starsRow }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var starsRow: some View {
        HStack(spacing: 4) {
            ForEach(1...5, id: \.self) { n in
                Button {
                    guard let id = song?.id else { return }
                    let next = stars == n ? 0 : n
                    stars = next
                    store.rate(id, stars: next)
                } label: {
                    Image(systemName: n <= stars ? "star.fill" : "star")
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundStyle(n <= stars ? store.colors.accent : .white.opacity(0.28))
                        .frame(width: 32, height: 32)
                        .contentShape(.rect)
                }
                .buttonStyle(Pressable.sign)
                .accessibilityLabel(count(n, "star"))
                .accessibilityAddTraits(n <= stars ? .isSelected : [])
            }
        }
        .padding(.leading, -5)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Rating")
    }

    /// Four pills, two by two, on a faint flat fill (never glass on the sheet's glass).
    @ViewBuilder
    private var pills: some View {
        if let song {
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
                pill("text.badge.plus", "Playlist", spoken: "Add to a playlist") { after { window.sheet = .addToPlaylist([song]) } }
                pill("dot.radiowaves.left.and.right", "Radio", spoken: "Start radio") {
                    let id = song.id
                    store.run { try $0.startRadio(id: id) }
                    store.playedWhenReady()
                    dismiss()
                }
                pill("square.stack", song.album.isEmpty ? "Album" : song.album, spoken: "Go to the album, \(song.album)") {
                    after { window.opening = .albumOf(song.id) }
                }
                pill("music.microphone", song.artist, spoken: "Go to the artist, \(song.artist)") {
                    after { window.opening = .artist(song.artist) }
                }
                .disabled(isPlaceholderArtist(song.artist))
            }
            .padding(.top, 18)
        }
    }

    private func pill(_ symbol: String, _ title: String, spoken: String, action: @escaping () -> Void) -> some View {
        Button { once(action) } label: {
            HStack(spacing: 10) {
                Image(systemName: symbol).font(.system(size: 17, weight: .medium)).opacity(0.86).frame(width: 22)
                Text(title).font(.needle(.subheadline).weight(.medium)).lineLimit(1).truncationMode(.tail)
                Spacer(minLength: 0)
            }
            .padding(.leading, 14).padding(.trailing, 16)
            .frame(height: 46)
            .background(.white.opacity(0.1), in: .capsule)
            .contentShape(.capsule)
        }
        .buttonStyle(Pressable())
        .accessibilityLabel(spoken)
    }

    /// The player: the look, then plain rows.
    @ViewBuilder
    private var playerPart: some View {
        Text("Look").font(.needle(.footnote)).foregroundStyle(.white.opacity(0.66)).padding(.top, wide ? 0 : 16)
        LookChoice(chosen: store.choices.bigLook, choose: setLook).padding(.top, 6).padding(.bottom, 6)
        if let song, !song.onComputer {
            loopRow(song)
        }
        if !store.onComputer {
            row("gauge.with.dots.needle.67percent", "Speed and pitch", detail: speedName(store.tempo.speed)) { after { window.sheet = .speed } }
            row("arrow.up.left.and.arrow.down.right", "Full screen") { after(fullScreen) }
        }
        // Where the pill of tools is not on the screen, its Sleep timer and Play on are here.
        if !pillShown {
            if !store.onComputer {
                row(store.sleep == nil ? "moon" : "moon.fill", "Sleep timer", detail: store.sleep.map { sleepClock($0.remaining) } ?? "Off") { after { window.sheet = .sleep } }
            }
            row("airplayaudio", "Play on") { after { window.sheet = .speakers } }
        }
    }

    /// The rare rows, each only where it applies.
    @ViewBuilder
    private var rare: some View {
        if let song, !song.onComputer {
            if song.fromServer {
                row("arrow.down.circle", "Keep on this \(Device.name.lowercased())") { store.keepOffline([song], keep: true); dismiss() }
                row("square.and.arrow.down", "Save to my music") { store.saveToPhone([song]); dismiss() }
            } else {
                row("tag", "Edit tags") { after { window.sheet = .tags([song.id]) } }
            }
            row("text.badge.checkmark", "Time the lyrics") { after { window.sheet = .timing(song) } }
            ForEach(store.songCommands) { command in
                row("puzzlepiece.extension", command.command.title) { store.runCommand(command, on: [song]); dismiss() }
            }
        }
    }

    private func loopRow(_ song: Song) -> some View {
        Menu {
            let at = min(store.position, max(0, song.duration - 1))
            Button("Start here (\(clock(at)))") { setLoop(at, loop.count == 2 ? max(loop[1], at + 1) : min(song.duration, at + 30)) }
            if loop.count == 2 {
                Button("End here (\(clock(at)))") { setLoop(loop[0], max(loop[0] + 1, at)) }
                Button("Stop repeating", role: .destructive, action: stopLoop)
            }
        } label: {
            rowLabel(nil, text: "A-B", "A-B loop", detail: loop.count == 2 ? "\(clock(loop[0]))–\(clock(loop[1]))" : "Off")
        }
        .accessibilityLabel("A-B loop")
        .accessibilityValue(loop.count == 2 ? "\(clock(loop[0])) to \(clock(loop[1]))" : "Off")
    }

    private func row(_ symbol: String?, text: String? = nil, _ title: String, detail: String? = nil, action: @escaping () -> Void) -> some View {
        Button { once(action) } label: { rowLabel(symbol, text: text, title, detail: detail) }
            .buttonStyle(.plain)
            .accessibilityValue(detail ?? "")
    }

    private func rowLabel(_ symbol: String?, text: String?, _ title: String, detail: String?) -> some View {
        HStack(spacing: 18) {
            Group {
                if let symbol { Image(systemName: symbol).font(.system(size: 19)) }
                else { Text(text ?? "").font(.system(size: 13, weight: .bold)).fixedSize() }
            }
            .opacity(0.7)
            .frame(width: 28)
            Text(title).font(.needle(.body)).lineLimit(1)
            Spacer(minLength: 8)
            if let detail { Text(detail).font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.66)).monospacedDigit() }
        }
        .frame(height: 50)
        .contentShape(.rect)
    }

    /// One press acts once.
    private func once(_ action: () -> Void) {
        guard !acted else { return }
        acted = true
        action()
        Task { try? await Task.sleep(for: .milliseconds(600)); acted = false }
    }

}

/// More's size: on a phone as tall as its parts (it scrolls inside when taller than the
/// screen); in a wide window short and fitted to its two columns.
private struct MoreSizing: ViewModifier {
    let wide: Bool
    let height: CGFloat

    func body(content: Content) -> some View {
        if wide {
            content.frame(width: 680).presentationSizing(.fitted)
        } else {
            content.presentationDetents([.height(min(height + 8, 760))])
        }
    }
}

/// The four looks as one row: the chosen one filled with the accent.
private struct LookChoice: View {
    @Environment(Store.self) private var store
    let chosen: String
    let choose: (String) -> Void
    @Namespace private var lit

    var body: some View {
        HStack(spacing: 0) {
            ForEach([("cover", "Cover"), ("record", "Record"), ("lyrics", "Lyrics"), ("visualizer", "Visualizer")], id: \.0) { id, name in
                let on = chosen == id
                Button { withMotion(.smooth(duration: 0.3)) { choose(id) } } label: {
                    Text(name)
                        .font(.needle(.subheadline).weight(.medium))
                        .lineLimit(1).minimumScaleFactor(0.8)
                        .foregroundStyle(on ? store.colors.onAccent : .white.opacity(0.7))
                        .frame(maxWidth: .infinity)
                        .frame(height: 36)
                        .background { if on { Capsule().fill(store.colors.accent).matchedGeometryEffect(id: "look", in: lit) } }
                        .contentShape(.capsule)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(4)
        .background(.black.opacity(0.2), in: .capsule)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Look")
    }
}

// MARK: - Up next

/// Up next on a phone, a glass sheet: its name, how many songs and how long, Shuffle (a pill in
/// the accent) and Add songs, then the songs (move them, take them out).
struct UpNextSheet: View {
    @Environment(Store.self) private var store
    let addSongs: () -> Void

    var body: some View {
        let songs = store.upNext
        let seconds = songs.reduce(0) { $0 + $1.duration }
        VStack(alignment: .leading, spacing: 0) {
            Text("Up next").font(.needle(.title2).bold()).padding(.top, 24)
            Text(songs.isEmpty ? "Nothing yet" : "\(count(songs.count, "song")) · \(Int((seconds / 60).rounded())) minutes")
                .font(.needle(.subheadline)).foregroundStyle(.white.opacity(0.66)).padding(.top, 2)
            UpNextActions(addSongs: addSongs).padding(.top, 14).padding(.bottom, 6)
            QueuePane(onPane: true, titled: false)
        }
        .padding(.horizontal, 26)
        .foregroundStyle(.white)
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .preferredColorScheme(.dark)
    }
}

/// Shuffle (a pill in the accent) and Add songs (a plain pill): flat fills, never glass on glass.
struct UpNextActions: View {
    @Environment(Store.self) private var store
    let addSongs: () -> Void
    @State private var shuffles = 0

    var body: some View {
        HStack(spacing: 10) {
            Button { shuffles += 1; store.shuffleButton() } label: {
                Label("Shuffle", systemImage: "shuffle")
                    .font(.needle(.subheadline).weight(.semibold))
                    .foregroundStyle(store.colors.onAccent)
                    .padding(.horizontal, 18).frame(height: 40)
                    .background(store.colors.accent, in: .capsule)
                    .pressBounce(shuffles)
            }
            .buttonStyle(Pressable())
            .accessibilityValue(store.choices.shuffleStays && !store.onComputer ? (store.shuffleOn ? "On" : "Off") : "")
            if !store.onComputer {
                Button(action: addSongs) {
                    Label("Add songs", systemImage: "plus")
                        .font(.needle(.subheadline).weight(.semibold))
                        .padding(.horizontal, 18).frame(height: 40)
                        .background(.white.opacity(0.12), in: .capsule)
                }
                .buttonStyle(Pressable())
            }
        }
    }
}

/// Add songs to Up next: find a song, then Play next or Add to the queue.
struct AddSongsSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var found: [Song] = []

    var body: some View {
        NavigationStack {
            List(found, id: \.id) { song in
                HStack(spacing: 12) {
                    Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                        .frame(width: 44, height: 44).clipShape(.soft(Corner.smallCover))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(song.title).lineLimit(1)
                        Text(song.artist).font(.needle(.subheadline)).foregroundStyle(.secondary).lineLimit(1)
                    }
                    Spacer(minLength: 8)
                    Button { store.playNext([song]) } label: { Image(systemName: "text.line.first.and.arrowtriangle.forward") }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Play \(song.title) next")
                    Button { store.enqueue([song]) } label: { Image(systemName: "plus.circle") }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Add \(song.title) to the queue")
                }
            }
            .listStyle(.plain)
            .overlay {
                if found.isEmpty {
                    ContentUnavailableView(query.isEmpty ? "Find songs to add" : "Nothing found", systemImage: "magnifyingglass")
                }
            }
            .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Songs, albums, artists")
            .navigationTitle("Add songs")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .task(id: query) {
                let words = query.trimmingCharacters(in: .whitespaces)
                guard !words.isEmpty else { found = []; return }
                try? await Task.sleep(for: .milliseconds(200))
                guard !Task.isCancelled else { return }
                found = Array((await store.load { try $0.search(query: words) } ?? []).prefix(80))
            }
        }
        .tint(store.colors.accent)
    }
}
