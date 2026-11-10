import SwiftUI
import UniformTypeIdentifiers

/// Needle on an iPad: one app with the iPhone, laid out by the room it has. A wide window (an
/// iPad's at regular width) gets the Mac's layout: the sidebar, the player floating over the
/// page's foot, pages laid out for the room. A narrow window (Slide Over, a third of Split View,
/// a small Stage Manager window) and every iPhone keep the phone's layout.

extension EnvironmentValues {
    /// The window (or the sheet) is wide: an iPad at regular width. Never on an iPhone, also
    /// not a large one on its side, which keeps its own layout.
    @Entry var wide = false
}

/// The iPad's layout in a wide window: the device is an iPad and the window, or the sheet, has
/// the regular width. A narrow window of an iPad is laid out as an iPhone.
struct WideReader: ViewModifier {
    @Environment(\.horizontalSizeClass) private var size

    static var isPad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    func body(content: Content) -> some View {
        content.environment(\.wide, Self.isPad && size == .regular)
    }
}

extension View {
    /// Sets `\.wide` from this place's size class (see `WideReader`).
    func readsWide() -> some View {
        modifier(WideReader())
    }

    /// In a wide window, rows and forms keep a readable width in the middle of the page (on an
    /// iPhone, and in a narrow window, nothing changes). Their backgrounds still fill the page.
    func readableWidth(_ most: CGFloat = 760) -> some View {
        modifier(ReadableWidth(most: most))
    }
}

private struct ReadableWidth: ViewModifier {
    @Environment(\.wide) private var wide
    let most: CGFloat
    @State private var width: CGFloat = 0

    func body(content: Content) -> some View {
        let side = wide ? max(0, (width - most) / 2) : 0
        content
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
            // Room at the sides as safe area: a list keeps its own insets, lines, and swipes.
            .safeAreaPadding(.horizontal, side)
    }
}

/// Grids of covers: wider tiles in a wide window, the phone's two to a row on an iPhone.
enum Tiles {
    /// The smallest a cover tile may be.
    static func least(_ wide: Bool) -> CGFloat { wide ? 172 : 150 }
    /// The gap between tiles.
    static func gap(_ wide: Bool) -> CGFloat { wide ? 22 : 14 }
    /// The page's margin at the sides of a grid.
    static func margin(_ wide: Bool) -> CGFloat { wide ? 28 : 16 }
    /// A tile's width on a shelf.
    static func shelf(_ wide: Bool) -> CGFloat { wide ? 180 : phoneShelf }
    /// A tile's width on a phone's shelf (an album tile's own width unless it is given one).
    static let phoneShelf: CGFloat = 150
    /// An artist's round picture on a shelf.
    static func bubble(_ wide: Bool) -> CGFloat { wide ? 128 : 104 }
}

// MARK: - Dragging songs onto a playlist

/// Songs or an album dragged in Needle, as plain text with a mark of Needle's own, so a drop
/// onto a playlist in the sidebar knows them (and text from elsewhere is left alone).
enum SongDrag {
    private static let songs = "needle-songs:"
    private static let album = "needle-album:"

    /// Dragged from Needle (not text from another app).
    static func isNeedle(_ item: String) -> Bool { item.hasPrefix(songs) || item.hasPrefix(album) }

    static func of(_ list: [Song]) -> String { songs + list.map(\.id).joined(separator: "\n") }
    static func of(_ chosen: Album) -> String { album + chosen.key }

    /// The songs a drop holds, read from the library.
    static func songs(in items: [String], store: Store) async -> [String] {
        var ids: [String] = []
        for item in items {
            if item.hasPrefix(songs) {
                ids += item.dropFirst(songs.count).split(separator: "\n").map(String.init)
            } else if item.hasPrefix(album) {
                let key = String(item.dropFirst(album.count))
                ids += (await store.load { try $0.albumSongs(key: key) } ?? []).map(\.id)
            }
        }
        return ids
    }
}

extension View {
    /// In a wide window, dragged onto a playlist in the sidebar, these songs go into it. On an
    /// iPhone a long press stays the songs' menu alone.
    func draggableSongs(_ songs: @autoclosure @escaping () -> [Song]) -> some View {
        modifier(WideDrag(payload: { SongDrag.of(songs()) }))
    }

    /// In a wide window, dragged onto a playlist in the sidebar, the album's songs go into it.
    func draggableAlbum(_ album: Album) -> some View {
        modifier(WideDrag(payload: { SongDrag.of(album) }))
    }
}

private struct WideDrag: ViewModifier {
    @Environment(\.wide) private var wide
    let payload: () -> String

    func body(content: Content) -> some View {
        if wide { content.draggable(payload()) } else { content }
    }
}
