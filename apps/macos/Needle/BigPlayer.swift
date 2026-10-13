import SwiftUI

/// The big player over the whole window (⌘P), and full screen (⌃⌘F): the cover large on its
/// blurred colours, with the lyrics, the songs up next, or the stems beside it.
struct BigPlayer: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @State private var resting = false
    @State private var moved = Date()
    @State private var showLyrics = true

    var body: some View {
        @Bindable var store = store
        let p = store.playback
        GeometryReader { geo in
            let wide = geo.size.width > 980
            ZStack {
                Color.black
                if let p, p.hasSong {
                    CoverBackdrop(path: p.artwork, seed: p.album + p.artist, moving: store.motion)
                } else {
                    store.colors.glow.opacity(0.3)
                }
                if let p, p.hasSong {
                    HStack(spacing: wide ? 60 : 32) {
                        VStack(spacing: 22) {
                            Cover(path: p.artwork, seed: p.album + p.artist, side: min(geo.size.height * 0.5, wide ? 460 : 340), corner: 14)
                                .shadow(color: .black.opacity(0.45), radius: 40, y: 18)
                            VStack(spacing: 4) {
                                Text(p.title).font(store.display(store.immersive ? 34 : 28)).lineLimit(2).multilineTextAlignment(.center)
                                Button(p.artist) { store.big = false; store.setImmersive(false); store.navigate(.artist(store.current?.artist ?? p.artist)) }
                                    .buttonStyle(.plain)
                                    .font(.title3)
                                    .foregroundStyle(.white.opacity(0.75))
                            }
                            .foregroundStyle(.white)
                            controls
                                .opacity(resting ? 0 : 1)
                        }
                        .frame(maxWidth: wide ? 480 : 360)
                        if wide && (!store.immersive || showLyrics) {
                            side
                                .frame(maxWidth: 560)
                                .frame(maxHeight: geo.size.height * 0.82)
                        }
                    }
                    .padding(48)
                } else {
                    VStack(spacing: 8) {
                        Text("Nothing playing").font(store.display(28)).foregroundStyle(.white)
                        Text("Pick something from your library.").foregroundStyle(.white.opacity(0.7))
                    }
                }
                VStack {
                    topBar.opacity(resting ? 0 : 1)
                    Spacer()
                }
            }
            .animation(store.motion ? .easeInOut(duration: 0.4) : nil, value: resting)
            .onContinuousHover { _ in rest(false) }
        }
        .ignoresSafeArea()
        .environment(\.colorScheme, .dark)
        .task(id: moved) {
            // Full screen: the controls rest a while after the pointer stops.
            guard store.immersive else { return }
            try? await Task.sleep(for: .seconds(3))
            if !Task.isCancelled { resting = true }
        }
    }

    private func rest(_ on: Bool) {
        if resting != on { resting = on }
        moved = Date()
    }

    private var controls: some View {
        VStack(spacing: 12) {
            SeekBar(light: true).frame(maxWidth: 440)
            HStack(spacing: 24) {
                if let c = store.current {
                    Heart(on: c.favorite, size: 16) { store.toggleFavorite([c]) }
                }
                Transport(play: 50, light: true)
                LoopButton()
            }
            VolumeControl(width: 160, light: true)
        }
    }

    private var topBar: some View {
        HStack(spacing: 14) {
            Button { store.big = false; store.setImmersive(false) } label: { Image(systemName: "chevron.down").font(.title3.weight(.semibold)) }
                .help("Close the player · Esc")
            Spacer()
            if store.immersive {
                Button { showLyrics.toggle() } label: { Image(systemName: showLyrics ? "quote.bubble.fill" : "quote.bubble") }
                    .help(showLyrics ? "Hide the lyrics" : "Show the lyrics")
            }
            Button { openWindow(id: "mini"); store.big = false } label: { Image(systemName: "pip") }.help("Mini player")
            Button { store.setImmersive(!store.immersive) } label: {
                Image(systemName: store.immersive ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right")
            }
            .help(store.immersive ? "Leave full screen · Esc" : "Full screen · ⌃⌘F")
        }
        .buttonStyle(.plain)
        .foregroundStyle(.white.opacity(0.85))
        .padding(.horizontal, 24)
        .padding(.leading, store.immersive ? 0 : 60)
        .padding(.top, store.immersive ? 24 : 16)
    }

    private var side: some View {
        @Bindable var store = store
        return VStack(spacing: 12) {
            if !store.immersive {
                Picker("", selection: $store.bigSide) {
                    ForEach(Store.BigSide.allCases, id: \.self) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 300)
            }
            Group {
                switch store.immersive ? .lyrics : store.bigSide {
                case .lyrics: LyricsView(kind: store.immersive ? .immersive : .big, light: true)
                case .queue: QueuePanel()
                case .stems:
                    ScrollView {
                        if let c = store.current { StemsPanel(track: c, big: true).padding(4) }
                    }
                }
            }
            .mask(LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.08),
                                         .init(color: .black, location: 0.9), .init(color: .clear, location: 1)],
                                 startPoint: .top, endPoint: .bottom))
        }
    }
}

/// A small window that stays out of the way, after Apple Music's: the cover fills it, and the
/// song and its controls show over the cover while the pointer is on it. Pin it to keep it
/// above other windows; the lyrics, what plays next, or what played last fold out beneath.
struct MiniPlayer: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    @AppStorage("mini-on-top") private var onTop = true
    @AppStorage("mini-view") private var view = "cover"
    @State private var hovering = false
    private let side: CGFloat = 300

    var body: some View {
        let p = store.playback
        VStack(spacing: 0) {
            cover(p)
            if view != "cover" { below }
        }
        .frame(width: side)
        .background {
            ZStack {
                Color.black
                if let p, p.hasSong {
                    CoverBackdrop(path: p.artwork, seed: p.album + p.artist, moving: false).opacity(0.9)
                }
                Color.black.opacity(0.3)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .onHover { inside in
            withAnimation(store.motion ? .easeOut(duration: 0.18) : nil) { hovering = inside }
        }
        .background(MiniWindow(floating: onTop))
        .containerBackground(.clear, for: .window)
        .preferredColorScheme(.dark)
        .tint(store.colors.accent)
    }

    private func cover(_ p: MacPlayback?) -> some View {
        ZStack {
            if let p, p.hasSong {
                Artwork(path: p.artwork, seed: p.album + p.artist, size: 600)
                    .frame(width: side, height: side)
            } else {
                Image(systemName: "music.note").font(.system(size: 54)).foregroundStyle(.white.opacity(0.3))
            }
            if hovering || p?.hasSong != true {
                controls(p).transition(.opacity)
            }
        }
        .frame(width: side, height: side)
        .clipped()
    }

    /// Over the cover: close, pin, and the menu at the top; the song, the seek bar, and the
    /// controls at the foot.
    private func controls(_ p: MacPlayback?) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 14) {
                Button { dismissWindow(id: "mini") } label: {
                    Image(systemName: "xmark")
                        .frame(width: 22, height: 22)
                        .background(.black.opacity(0.35), in: Circle())
                }
                .help("Close the mini player")
                Spacer()
                Button { onTop.toggle() } label: {
                    Image(systemName: onTop ? "pin.fill" : "pin").rotationEffect(.degrees(45))
                }
                .help(onTop ? "Unpin: other windows can cover the mini player" : "Pin: keep the mini player above other windows")
                menu
            }
            .font(.system(size: 14, weight: .semibold))
            .padding(.horizontal, 12)
            .padding(.top, 10)
            .padding(.bottom, 30)
            .background(LinearGradient(colors: [.black.opacity(0.5), .clear], startPoint: .top, endPoint: .bottom))
            Spacer(minLength: 0)
            VStack(spacing: 8) {
                VStack(spacing: 1) {
                    Text(p?.hasSong == true ? p!.title : "Nothing playing").font(.headline).lineLimit(1)
                    if let p, p.hasSong {
                        Text(p.artist).font(.callout).foregroundStyle(.white.opacity(0.75)).lineLimit(1)
                    }
                }
                SeekBar(small: true, light: true)
                HStack {
                    fold("lyrics", icon: "quote.bubble", help: "Lyrics")
                    Spacer()
                    Transport(play: 38, compact: true, light: true)
                    Spacer()
                    fold("queue", icon: "list.bullet", help: "Playing next")
                }
                .font(.system(size: 14))
            }
            .padding(.horizontal, 16)
            .padding(.top, 44)
            .padding(.bottom, 14)
            .background(LinearGradient(colors: [.clear, .black.opacity(0.75)], startPoint: .top, endPoint: .bottom))
        }
        .foregroundStyle(.white)
        .buttonStyle(.plain)
    }

    /// A button that folds a view out beneath the cover, or folds it away.
    private func fold(_ name: String, icon: String, help: String) -> some View {
        Button { view = view == name ? "cover" : name } label: {
            Image(systemName: icon)
                .foregroundStyle(view == name ? store.colors.accent : .white.opacity(0.85))
                .frame(width: 26, height: 26)
                .contentShape(Rectangle())
        }
        .help(view == name ? "Hide the \(help.lowercased())" : "Show the \(help.lowercased())")
    }

    private var menu: some View {
        Menu {
            Picker("Show beneath", selection: $view) {
                Text("Nothing").tag("cover")
                Text("Lyrics").tag("lyrics")
                Text("Playing Next").tag("queue")
                Text("History").tag("history")
            }
            Toggle("Keep on Top", isOn: $onTop)
            Divider()
            Button("Open Needle") {
                openWindow(id: "main")
                dismissWindow(id: "mini")
            }
        } label: {
            Image(systemName: "ellipsis")
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
    }

    private var below: some View {
        VStack(spacing: 0) {
            Picker("", selection: $view) {
                Text("Lyrics").tag("lyrics")
                Text("Up Next").tag("queue")
                Text("History").tag("history")
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(10)
            Group {
                switch view {
                case "lyrics": LyricsView(kind: .mini, light: true).padding(.horizontal, 14)
                case "history": MiniHistory()
                default: QueuePanel()
                }
            }
            .frame(maxHeight: .infinity)
        }
        .frame(width: side, height: 280)
    }
}

/// The mini player's window (it has no title bar): moved by dragging anywhere, with a
/// shadow, kept above other windows while pinned, and shown over full-screen apps.
struct MiniWindow: NSViewRepresentable {
    let floating: Bool

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            guard let window = view.window else { return }
            window.level = floating ? .floating : .normal
            window.isMovableByWindowBackground = true
            window.hasShadow = true
            window.collectionBehavior.insert(.fullScreenAuxiliary)
        }
    }
}

/// The songs played last, in the mini player; double-click one to play it again.
struct MiniHistory: View {
    private var store: Store { .shared }
    @State private var items: [HistoryItem] = []

    var body: some View {
        List(Array(items.enumerated()), id: \.offset) { _, item in
            HStack(spacing: 10) {
                Cover(path: item.song?.artwork, seed: item.album + item.artist, side: 30, corner: 4)
                VStack(alignment: .leading, spacing: 1) {
                    Text(item.title).lineLimit(1)
                    Text(item.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                Text(Date(timeIntervalSince1970: TimeInterval(item.startedAt)), format: .relative(presentation: .named))
                    .font(.caption2).foregroundStyle(.secondary)
            }
            .contentShape(Rectangle())
            .onTapGesture(count: 2) { if let song = item.song { store.play([song.asTrack], reason: "Played again from your history") } }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .task(id: store.playback?.current) {
            items = await store.load { try $0.history(offset: 0, limit: 40) } ?? []
        }
    }
}
