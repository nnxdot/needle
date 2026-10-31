#if DEBUG
import SwiftUI

/// Test builds: pictures of a view drawn on its own (ImageRenderer), with no window, for when
/// the window cannot be seen (another app full screen on this Mac): from the app's start, not
/// the window's.
/// - `--test-play <album>`: plays that album.
/// - `--render-crate <path.png>`: the record crate, 14 seconds in.
/// - `--render-visualizer <path.png>`: the visualizer in a big player's frame, 16 seconds in,
///   and once more a second later (`<path>-2.png`).
/// - `--render-big <path.png>`: the real big player, 16 seconds in, and a second later.
/// - `--test-mcp-off <seconds>`: turns AI assistants off that long after the start.
/// - `--test-spectrum <file>`: the core's spectrum twice a second for 12 seconds.
@MainActor
enum TestRenders {
    static func start() {
        let args = CommandLine.arguments
        let value = { (flag: String) in args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil } }
        let store = Store.shared
        if let title = value("--test-play") {
            Task {
                try? await Task.sleep(for: .seconds(4))
                let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
                guard let album = albums.first(where: { $0.album == title }) else { return }
                _ = await store.ask { try $0.playView(query: album.query, playlist: nil, filter: "", start: 0, shuffle: false, reason: "From the album \(title)") }
            }
        }
        if let path = value("--render-crate") {
            Task {
                try? await Task.sleep(for: .seconds(14))
                let albums = JSON.decode([AlbumSummary].self, await store.load { try $0.albumsJson(query: "") }) ?? []
                render(RecordCrate(albums: albums)
                    .frame(width: 1000, height: 760)
                    .background(store.colors.canvas), to: path)
            }
        }
        if let path = value("--render-visualizer") {
            Task {
                try? await Task.sleep(for: .seconds(16))
                for file in [path, path.replacingOccurrences(of: ".png", with: "-2.png")] {
                    // A big player's frame: the cover's colours behind, the visualizer in the
                    // cover's place (the old one filled the frame itself).
                    render(ZStack {
                        LinearGradient(colors: [store.colors.glow.opacity(0.55), .black], startPoint: .top, endPoint: .bottom)
                        Visualizer().frame(width: 480)
                    }
                    .frame(width: 1400, height: 890), to: file)
                    try? await Task.sleep(for: .seconds(1))
                }
            }
        }
        if let path = value("--render-big") {
            Task {
                try? await Task.sleep(for: .seconds(16))
                // The cover's colours read first: a picture runs no `.task`.
                if let p = store.playback { _ = await Visualizer.coverColor(p.artwork, seed: p.album + p.artist) }
                for file in [path, path.replacingOccurrences(of: ".png", with: "-2.png")] {
                    // The real big player, in its look (`big_look`), at a window's size.
                    render(BigPlayer().environment(store).frame(width: 1320, height: 860), to: file)
                    try? await Task.sleep(for: .seconds(1))
                }
            }
        }
        if let seconds = value("--test-mcp-off").flatMap(Double.init) {
            Task {
                try? await Task.sleep(for: .seconds(seconds))
                store.set(["mcp": false])
            }
        }
        if let file = value("--test-spectrum") {
            Task {
                var lines: [String] = []
                for _ in 0..<24 {
                    try? await Task.sleep(for: .milliseconds(500))
                    let bands = store.core.spectrum(bands: 48)
                    let most = bands.max() ?? 0, mean = bands.reduce(0, +) / Float(max(bands.count, 1))
                    lines.append(String(format: "max %.2f  mean %.2f  low %.2f  high %.2f", most, mean, bands.first ?? 0, bands.last ?? 0))
                }
                try? lines.joined(separator: "\n").write(toFile: file, atomically: true, encoding: .utf8)
            }
        }
    }

    private static func render(_ view: some View, to path: String) {
        let renderer = ImageRenderer(content: view.environment(\.colorScheme, Store.shared.colors.dark ? .dark : .light))
        renderer.scale = 1
        guard let tiff = renderer.nsImage?.tiffRepresentation,
              let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { return }
        try? png.write(to: URL(fileURLWithPath: path))
    }
}
#endif
