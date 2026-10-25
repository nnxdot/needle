import SwiftUI

/// Albums as records in a box: the one in front stands up, the ones behind peek over it, and a
/// drag down flips the front one toward you to show the next. A tap on a record behind jumps
/// to it; the front one opens its album.
struct RecordCrateView: View {
    @Environment(Store.self) private var store
    @State private var albums: [Album]?
    @State private var index = 0
    /// How far the finger has pulled, in points (down flips forward).
    @State private var drag: CGFloat = 0

    /// The pull that flips one record.
    private let step: CGFloat = 70

    var body: some View {
        GeometryReader { geo in
            let side = min(geo.size.width - 72, geo.size.height * 0.48, 340)
            VStack(spacing: 22) {
                Spacer(minLength: 0)
                if let albums, !albums.isEmpty {
                    crate(albums, side: side)
                    info(albums[index])
                }
                Spacer(minLength: 0)
            }
            .frame(maxWidth: .infinity)
        }
        .overlay {
            if albums == nil { ProgressView() } else if albums?.isEmpty == true { ContentUnavailableView("No albums", systemImage: "square.stack") }
        }
        .navigationTitle("Record crate")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { shuffle() } label: { Image(systemName: "shuffle") }
                    .accessibilityLabel("Shuffle the crate")
                    .disabled(albums?.isEmpty ?? true)
            }
        }
        .sensoryFeedback(.selection, trigger: index) { _, _ in store.app.haptics }
        .task(id: store.libraryVersion) {
            let found = await store.load { try $0.albums() } ?? []
            albums = found
            index = min(index, max(0, found.count - 1))
        }
    }

    /// Where the crate stands, in records: the front one's place plus the pull so far.
    private var progress: Double { Double(index) + Double(drag / step) }

    private func crate(_ albums: [Album], side: CGFloat) -> some View {
        let low = max(0, index - 2), high = min(albums.count - 1, index + 6)
        return ZStack(alignment: .bottom) {
            ForEach(low...high, id: \.self) { i in
                record(albums[i], at: Double(i) - progress, side: side)
                    .onTapGesture {
                        if i == index { store.opening = .album(albums[i]) } else { flip(to: i, count: albums.count) }
                    }
            }
            // The front of the box: the records stand in it.
            RoundedRectangle(cornerRadius: 14)
                .fill(store.colors.card.opacity(0.92))
                .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(.white.opacity(0.08)))
                .frame(width: side + 36, height: side * 0.3)
                .shadow(color: .black.opacity(0.25), radius: 12, y: -2)
                .zIndex(0.25)
                .allowsHitTesting(false)
        }
        .frame(width: side + 36, height: side * 1.55, alignment: .bottom)
        .contentShape(.rect)
        .gesture(
            DragGesture(minimumDistance: 4)
                .onChanged { drag = $0.translation.height }
                .onEnded { value in
                    let target = Int((Double(index) + Double(value.predictedEndTranslation.height / step)).rounded())
                    flip(to: target, count: albums.count)
                }
        )
        .accessibilityElement()
        .accessibilityLabel("Record crate")
        .accessibilityValue("\(albums[index].title), by \(albums[index].artist), \(index + 1) of \(albums.count)")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: flip(to: index + 1, count: albums.count)
            case .decrement: flip(to: index - 1, count: albums.count)
            @unknown default: break
            }
        }
        .accessibilityAction { store.opening = .album(albums[index]) }
    }

    /// One record at `d` places from the front: behind it (d > 0) it sits higher and smaller;
    /// flipped forward (d < 0) it tips toward you and fades.
    private func record(_ album: Album, at d: Double, side: CGFloat) -> some View {
        let forward = min(1, max(0, -d))
        let behind = max(0, d)
        return Artwork(path: album.artwork, seed: album.title + album.artist, size: 600)
            .frame(width: side, height: side)
            .clipShape(.rect(cornerRadius: 6))
            .shadow(color: .black.opacity(0.35), radius: 10, y: 6)
            .brightness(-min(behind, 5) * 0.07)
            .scaleEffect(1 - min(behind, 6) * 0.045, anchor: .bottom)
            .rotation3DEffect(.degrees(10 - forward * 80), axis: (x: 1, y: 0, z: 0), anchor: .bottom, perspective: 0.45)
            .offset(y: -behind * side * 0.12 + forward * side * 0.18 - side * 0.12)
            .opacity(d < -1 ? max(0, 2 + d) * 0.5 : 1 - forward * 0.5)
            .zIndex(-d)
    }

    private func info(_ album: Album) -> some View {
        VStack(spacing: 12) {
            VStack(spacing: 3) {
                Text(album.title).font(.title3.bold()).lineLimit(1)
                Text(album.artist).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
            .padding(.horizontal)
            HStack(spacing: 12) {
                Button { play(album) } label: {
                    Label("Play", systemImage: "play.fill").padding(.horizontal, 10).padding(.vertical, 4)
                }
                .buttonStyle(.glassProminent)
                .foregroundStyle(store.colors.onAccent)
                Button { store.opening = .album(album) } label: {
                    Label("Open", systemImage: "square.stack").padding(.horizontal, 10).padding(.vertical, 4)
                }
                .buttonStyle(.glass)
            }
        }
        .animation(nil, value: index)
    }

    private func flip(to target: Int, count: Int) {
        let next = min(max(0, target), count - 1)
        withAnimation(.spring(response: 0.45, dampingFraction: 0.82)) {
            index = next
            drag = 0
        }
    }

    private func shuffle() {
        withAnimation(.easeInOut(duration: 0.3)) {
            albums?.shuffle()
            index = 0
            drag = 0
        }
    }

    private func play(_ album: Album) {
        let key = album.key
        Task { if let songs = await store.load({ try $0.albumSongs(key: key) }), !songs.isEmpty { store.play(songs) } }
    }
}
