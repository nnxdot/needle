import AVKit
import MediaPlayer
import SwiftUI

/// The song playing, on the tab bar: tap it for the full player. When the tab bar is small
/// (scrolled down), only the song's name and play or pause are left.
struct MiniPlayer: View {
    @Environment(Store.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        let song = store.playback?.current
        let playing = store.playback?.playing == true
        HStack(spacing: 10) {
            Artwork(path: song?.artwork, seed: (song?.album ?? "") + (song?.artist ?? ""), size: 120)
                .frame(width: 34, height: 34)
                .clipShape(.circle)
            VStack(alignment: .leading, spacing: 0) {
                Text(song?.title ?? "Not Playing").font(.subheadline.weight(.semibold)).lineLimit(1)
                if placement != .inline, let song {
                    Text(song.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button { store.toggle() } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.title3)
                    .contentTransition(.symbolEffect(.replace))
                    .frame(width: 36, height: 36)
            }
            .disabled(song == nil)
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
    }
}

/// The full player: the cover over its own colours, the seek bar, the controls, and, a tap away,
/// the lyrics and the songs up next.
struct NowPlayingView: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var pane = Pane.cover
    @State private var favorite = false

    enum Pane { case cover, lyrics, queue }

    var body: some View {
        let song = store.playback?.current
        let seed = (song?.album ?? "") + (song?.artist ?? "")
        ZStack {
            Color.black.ignoresSafeArea()
            CoverBackdrop(path: song?.artwork, seed: seed)
            VStack(spacing: 0) {
                top(song)
                switch pane {
                case .cover:
                    Spacer(minLength: 8)
                    cover(song, seed: seed)
                    Spacer(minLength: 16)
                    title(song)
                case .lyrics:
                    small(song, seed: seed)
                    LyricsPane(song: song)
                case .queue:
                    small(song, seed: seed)
                    QueuePane()
                }
                SeekBar(duration: song?.duration ?? 0).padding(.top, 16)
                controls
                VolumeBar().frame(height: 34).padding(.top, 6)
                bottom.padding(.top, 14)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 8)
        }
        .animation(.spring(response: 0.45, dampingFraction: 0.85), value: pane)
        .task(id: song?.id) {
            guard let id = song?.id else { return }
            favorite = await store.load { $0.isFavorite(id: id) } ?? false
        }
        .onChange(of: song == nil) { _, gone in if gone { dismiss() } }
    }

    private func top(_ song: Song?) -> some View {
        HStack {
            Button { dismiss() } label: { Image(systemName: "chevron.down").font(.headline).frame(width: 30, height: 30) }
                .buttonStyle(.glass).buttonBorderShape(.circle)
            Spacer()
            VStack(spacing: 1) {
                Text("Playing from").font(.caption2).foregroundStyle(.secondary)
                Text(song?.album ?? "").font(.footnote.weight(.semibold)).lineLimit(1)
            }
            Spacer()
            Menu {
                if let song { SongMenu(song: song) }
            } label: {
                Image(systemName: "ellipsis").font(.headline).frame(width: 30, height: 30)
            }
            .buttonStyle(.glass).buttonBorderShape(.circle)
        }
        .padding(.top, 8)
    }

    private func cover(_ song: Song?, seed: String) -> some View {
        let playing = store.playback?.playing == true
        return Artwork(path: song?.artwork, seed: seed, size: 1000)
            .aspectRatio(1, contentMode: .fit)
            .clipShape(.rect(cornerRadius: 18))
            .shadow(color: .black.opacity(0.5), radius: 30, y: 20)
            .scaleEffect(playing ? 1 : 0.84)
            .animation(.spring(response: 0.55, dampingFraction: 0.62), value: playing)
            .frame(maxHeight: 360)
    }

    private func title(_ song: Song?) -> some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "").font(.title3.bold()).lineLimit(1)
                Text(song?.artist ?? "").font(.title3).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            favoriteButton(song)
        }
    }

    private func favoriteButton(_ song: Song?) -> some View {
        Button {
            guard let id = song?.id else { return }
            favorite.toggle()
            store.run { _ = try $0.toggleFavorite(id: id) }
        } label: {
            Image(systemName: favorite ? "star.fill" : "star").font(.headline).frame(width: 30, height: 30)
                .contentTransition(.symbolEffect(.replace))
        }
        .buttonStyle(.glass).buttonBorderShape(.circle)
    }

    /// The song, small, above the lyrics or the queue.
    private func small(_ song: Song?, seed: String) -> some View {
        HStack(spacing: 12) {
            Artwork(path: song?.artwork, seed: seed, size: 200)
                .frame(width: 56, height: 56)
                .clipShape(.rect(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 2) {
                Text(song?.title ?? "").font(.headline).lineLimit(1)
                Text(song?.artist ?? "").font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            favoriteButton(song)
        }
        .padding(.vertical, 14)
        .transition(.opacity)
    }

    private var controls: some View {
        let playing = store.playback?.playing == true
        return HStack {
            Spacer()
            BigButton(symbol: "backward.fill", size: 34) { store.previous() }
            Spacer()
            BigButton(symbol: playing ? "pause.fill" : "play.fill", size: 46) { store.toggle() }
            Spacer()
            BigButton(symbol: "forward.fill", size: 34) { store.next() }
            Spacer()
        }
        .padding(.top, 10)
    }

    private var bottom: some View {
        GlassEffectContainer(spacing: 20) {
            HStack {
                paneButton(.lyrics, symbol: "quote.bubble")
                Spacer()
                RoutePicker().frame(width: 48, height: 48).glassEffect(.regular.interactive(), in: .circle)
                Spacer()
                paneButton(.queue, symbol: "list.bullet")
            }
            .padding(.horizontal, 24)
        }
    }

    private func paneButton(_ which: Pane, symbol: String) -> some View {
        let on = pane == which
        return Button { pane = on ? .cover : which } label: {
            Image(systemName: symbol).font(.title3)
                .foregroundStyle(on ? Color.black : .white)
                .frame(width: 48, height: 48)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .glassEffect(on ? .regular.tint(.white).interactive() : .regular.interactive(), in: .circle)
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

/// The seek bar: a strip of glass that grows while it is dragged.
struct SeekBar: View {
    @Environment(Store.self) private var store
    let duration: Double
    @State private var dragged: Double?

    var body: some View {
        let position = dragged ?? store.playback?.position ?? 0
        let part = duration > 0 ? min(1, max(0, position / duration)) : 0
        VStack(spacing: 6) {
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(.white.opacity(0.18))
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
            HStack {
                Text(clock(position))
                Spacer()
                Text("-" + clock(max(0, duration - position)))
            }
            .font(.caption.weight(.semibold).monospacedDigit())
            .foregroundStyle(.secondary)
        }
    }
}

/// The phone's own volume, as Control Center sets it.
struct VolumeBar: UIViewRepresentable {
    func makeUIView(context: Context) -> MPVolumeView {
        let view = MPVolumeView(frame: .zero)
        view.tintColor = .white
        return view
    }

    func updateUIView(_ view: MPVolumeView, context: Context) {}
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

/// Timed lyrics that follow the song (tap a line to go there), or the plain words.
struct LyricsPane: View {
    @Environment(Store.self) private var store
    let song: Song?
    @State private var lyrics: Lyrics?
    @State private var loaded = false

    var body: some View {
        Group {
            if let lyrics, !lyrics.lines.isEmpty {
                timed(lyrics.lines)
            } else if let lyrics, !lyrics.plain.isEmpty {
                ScrollView {
                    Text(lyrics.plain).font(.title3.weight(.semibold)).frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 20)
                }
            } else if loaded {
                ContentUnavailableView(lyrics?.instrumental == true ? "Instrumental" : "No lyrics for this song", systemImage: "quote.bubble")
            } else {
                ProgressView().frame(maxHeight: .infinity)
            }
        }
        .frame(maxHeight: .infinity)
        .mask(LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.08), .init(color: .black, location: 0.8), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom))
        .task(id: song?.id) {
            loaded = false
            lyrics = nil
            guard let id = song?.id else { return }
            lyrics = await store.load { try $0.lyrics(id: id) } ?? nil
            loaded = true
        }
    }

    private func timed(_ lines: [LyricLine]) -> some View {
        let position = store.playback?.position ?? 0
        let current = lines.lastIndex { $0.time <= position + 0.2 } ?? -1
        return ScrollViewReader { reader in
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 22) {
                    Color.clear.frame(height: 40)
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        Text(line.text.isEmpty ? "♪" : line.text)
                            .font(.system(size: 28, weight: .heavy))
                            .opacity(i == current ? 1 : 0.32)
                            .blur(radius: i == current ? 0 : 1)
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
    }
}

/// The songs up next: tap one to play it, swipe to take it out, drag to move it.
struct QueuePane: View {
    @Environment(Store.self) private var store

    var body: some View {
        VStack(spacing: 10) {
            HStack {
                Text("Up Next").font(.headline)
                Spacer()
                GlassEffectContainer {
                    HStack(spacing: 8) {
                        Button { store.shuffle() } label: { Image(systemName: "shuffle").frame(width: 22, height: 22) }
                            .buttonStyle(.glass).buttonBorderShape(.circle)
                        Button { store.cycleRepeat() } label: {
                            Image(systemName: store.playback?.repeat == .one ? "repeat.1" : "repeat").frame(width: 22, height: 22)
                        }
                        .buttonStyle(store.playback?.repeat == .off ? AnyPrimitiveButtonStyle(.glass) : AnyPrimitiveButtonStyle(.glassProminent))
                        .buttonBorderShape(.circle)
                    }
                }
            }
            if store.upNext.isEmpty {
                ContentUnavailableView("Nothing up next", systemImage: "list.bullet").frame(maxHeight: .infinity)
            } else {
                List {
                    ForEach(Array(store.upNext.enumerated()), id: \.offset) { i, song in
                        Button { store.jump(i) } label: {
                            HStack(spacing: 12) {
                                Artwork(path: song.artwork, seed: song.album + song.artist, size: 120)
                                    .frame(width: 44, height: 44).clipShape(.rect(cornerRadius: 7))
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
                        .listRowInsets(EdgeInsets(top: 6, leading: 0, bottom: 6, trailing: 0))
                    }
                    .onDelete { rows in rows.sorted(by: >).forEach { store.core.removeUpNext(index: UInt32($0)) } }
                    .onMove { from, to in
                        guard let first = from.first else { return }
                        // The core moves to a place in the list as it is after taking the song out.
                        store.core.moveUpNext(from: UInt32(first), to: UInt32(to > first ? to - 1 : to))
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .environment(\.editMode, .constant(.inactive))
            }
        }
        .frame(maxHeight: .infinity)
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
