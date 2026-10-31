import SwiftUI

/// Albums as records in a box: the one in front stands up, the ones behind peek over it. A drag
/// down, or the arrow keys, flips the front one toward you to show the next; a click on a record
/// behind jumps to it, and on the front one opens its album.
struct RecordCrate: View {
    private var store: Store { .shared }
    let albums: [AlbumSummary]
    @State private var index = 0
    /// How far the pointer has pulled, in points (down flips forward).
    @State private var drag: CGFloat = 0
    @FocusState private var focused: Bool

    /// The pull that flips one record.
    private let step: CGFloat = 60

    var body: some View {
        GeometryReader { geo in
            let side = min(geo.size.height * 0.5, geo.size.width - 120, 380)
            VStack(spacing: 22) {
                Spacer(minLength: 0)
                if !albums.isEmpty {
                    let i = min(index, albums.count - 1)
                    crate(side: side)
                    info(albums[i])
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity)
        }
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onAppear { focused = true }
        .onChange(of: albums) { _, now in index = min(index, max(0, now.count - 1)) }
        .onKeyPress(.downArrow) { flip(by: 1); return .handled }
        .onKeyPress(.rightArrow) { flip(by: 1); return .handled }
        .onKeyPress(.upArrow) { flip(by: -1); return .handled }
        .onKeyPress(.leftArrow) { flip(by: -1); return .handled }
        .onKeyPress(.return) { open(); return .handled }
    }

    /// Where the crate stands, in records: the front one's place plus the pull so far.
    private var progress: Double { Double(index) + Double(drag / step) }

    private func crate(side: CGFloat) -> some View {
        let low = max(0, index - 2), high = min(albums.count - 1, index + 6)
        return ZStack(alignment: .bottom) {
            ForEach(low...high, id: \.self) { i in
                record(albums[i], at: Double(i) - progress, side: side)
                    .onTapGesture { if i == index { open() } else { flip(to: i) } }
            }
            // No box front, as on Windows: it showed as a pale bar under the front cover.
        }
        .frame(width: side + 40, height: side * 1.55, alignment: .bottom)
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 4)
                .onChanged { drag = $0.translation.height }
                .onEnded { value in
                    flip(to: Int((Double(index) + Double(value.predictedEndTranslation.height / step)).rounded()))
                }
        )
        .accessibilityElement()
        .accessibilityLabel("Record crate")
        .accessibilityValue("\(albums[index].title), by \(albums[index].artist), \(index + 1) of \(albums.count)")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: flip(by: 1)
            case .decrement: flip(by: -1)
            @unknown default: break
            }
        }
        .accessibilityAction { open() }
    }

    /// One record at `d` places from the front: behind it (d > 0) it sits higher and smaller;
    /// flipped forward (d < 0) it tips toward you and fades.
    private func record(_ album: AlbumSummary, at d: Double, side: CGFloat) -> some View {
        let forward = min(1, max(0, -d))
        let behind = max(0, d)
        return Cover(path: album.artwork, seed: album.album + album.artist, side: side, corner: 7)
            .shadow(color: .black.opacity(0.35), radius: 12, y: 6)
            .brightness(-min(behind, 5) * 0.07)
            .scaleEffect(1 - min(behind, 6) * 0.045, anchor: .bottom)
            .rotation3DEffect(.degrees(6 - forward * 80), axis: (x: 1, y: 0, z: 0), anchor: .bottom, perspective: 0.45)
            .offset(y: -behind * side * 0.12 + forward * side * 0.18 - side * 0.12)
            .opacity(d < -1 ? max(0, 2 + d) * 0.5 : 1 - forward * 0.5)
            .zIndex(-d)
    }

    private func info(_ album: AlbumSummary) -> some View {
        VStack(spacing: 12) {
            VStack(spacing: 3) {
                Text(album.title).font(store.display(22)).lineLimit(1)
                Text(album.year > 0 ? "\(album.artist) · \(album.year)" : album.artist)
                    .foregroundStyle(store.colors.ink2).lineLimit(1)
            }
            HStack(spacing: 10) {
                Button { play(album) } label: { Label("Play", systemImage: "play.fill") }
                    .glassButton(prominent: true)
                    .controlSize(.large)
                Button { open() } label: { Label("Open", systemImage: "square.stack") }
                    .glassButton(prominent: false)
                    .controlSize(.large)
            }
            Text("Drag down or press ↓ to flip").font(.needle(.caption)).foregroundStyle(store.colors.ink3)
        }
        .animation(nil, value: index)
    }

    private func flip(by n: Int) { flip(to: index + n) }

    private func flip(to target: Int) {
        let next = min(max(0, target), albums.count - 1)
        withAnimation(store.motion ? .spring(response: 0.45, dampingFraction: 0.82) : nil) {
            index = next
            drag = 0
        }
    }

    private func open() {
        guard albums.indices.contains(index) else { return }
        store.navigate(albums[index].route)
    }

    private func play(_ album: AlbumSummary) {
        let reason = "From the album \(album.album)"
        Task { _ = await store.ask { try $0.playView(query: album.query, playlist: nil, filter: "", start: 0, shuffle: false, reason: reason) } }
    }
}
