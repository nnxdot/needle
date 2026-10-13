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
        .padding(.top, store.immersive ? 24 : 14)
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

/// A small window that stays out of the way: the cover, the song, and the controls; its lyrics
/// or what plays next beneath when it is taller.
struct MiniPlayer: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    @AppStorage("mini-on-top") private var onTop = true
    @AppStorage("mini-view") private var view = "cover"

    var body: some View {
        let p = store.playback
        VStack(spacing: 0) {
            ZStack(alignment: .bottom) {
                if let p, p.hasSong {
                    Artwork(path: p.artwork, seed: p.album + p.artist, size: 600)
                        .frame(width: 300, height: 300)
                } else {
                    store.colors.raised.frame(width: 300, height: 300)
                        .overlay { Text("Nothing playing").foregroundStyle(.secondary) }
                }
                VStack(spacing: 6) {
                    if let p, p.hasSong {
                        Text(p.title).font(.headline).lineLimit(1)
                        Text(p.artist).font(.callout).foregroundStyle(.white.opacity(0.75)).lineLimit(1)
                    }
                    SeekBar(small: true, light: true)
                    Transport(play: 34, compact: true, light: true)
                }
                .foregroundStyle(.white)
                .padding(14)
                .frame(maxWidth: .infinity)
                .background(LinearGradient(colors: [.clear, .black.opacity(0.75)], startPoint: .top, endPoint: .bottom))
            }
            .frame(width: 300, height: 300)
            if view != "cover" {
                Group {
                    if view == "lyrics" {
                        LyricsView(kind: .mini).padding(.horizontal, 14)
                    } else if view == "history" {
                        MiniHistory()
                    } else {
                        QueuePanel()
                    }
                }
                .frame(width: 300, height: 260)
            }
        }
        .overlay(alignment: .topTrailing) {
            HStack(spacing: 6) {
                Menu {
                    Picker("Show", selection: $view) {
                        Text("Cover").tag("cover")
                        Text("Lyrics").tag("lyrics")
                        Text("Playing next").tag("queue")
                        Text("History").tag("history")
                    }
                    Toggle("Keep on top", isOn: $onTop)
                    Divider()
                    Button("Back to the full window") {
                        openWindow(id: "main")
                        dismissWindow(id: "mini")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle.fill").font(.title3).foregroundStyle(.white.opacity(0.85))
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
            }
            .padding(10)
        }
        .background(WindowLevel(floating: onTop))
        .environment(\.colorScheme, .dark)
        .tint(store.colors.accent)
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

/// Keeps its window above the others while `floating`.
struct WindowLevel: NSViewRepresentable {
    let floating: Bool

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            view.window?.level = floating ? .floating : .normal
            view.window?.isMovableByWindowBackground = true
        }
    }
}
