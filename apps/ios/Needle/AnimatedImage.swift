import ImageIO
import SwiftUI

/// An animated WebP or GIF, its frames shown in turn with their own delays, over and over: a
/// moving cover that is a picture, not a video. It fills its frame (cropped), as the videos do.
struct AnimatedImage: View {
    let url: URL
    @State private var frames: Frames?

    /// Whether a moving cover's file is a picture (WebP or GIF), not a video.
    static func isPicture(_ url: URL) -> Bool {
        ["webp", "gif"].contains(url.pathExtension.lowercased())
    }

    var body: some View {
        Group {
            if let frames, !frames.images.isEmpty {
                TimelineView(.animation(minimumInterval: frames.shortest)) { context in
                    Image(decorative: frames.image(at: context.date.timeIntervalSinceReferenceDate), scale: 1)
                        .resizable()
                        .scaledToFill()
                }
            } else {
                Color.clear
            }
        }
        .task(id: url) {
            let url = url
            frames = await Task.detached(priority: .userInitiated) { Frames.load(url) }.value
            #if DEBUG
            // Test builds: what was read, for checking from a shell.
            let line = "\(url.lastPathComponent): \(frames?.images.count ?? 0) frames, \(String(format: "%.2f", frames?.total ?? 0)) s a loop\n"
            try? line.write(toFile: NSTemporaryDirectory() + "needle-animated.log", atomically: true, encoding: .utf8)
            #endif
        }
        .accessibilityHidden(true)
    }

    /// The frames, each with the moment it ends within one loop.
    final class Frames: @unchecked Sendable {
        let images: [CGImage]
        let ends: [Double]
        let total: Double
        let shortest: Double

        init(images: [CGImage], delays: [Double]) {
            self.images = images
            var at = 0.0
            ends = delays.map { at += $0; return at }
            total = max(at, 0.01)
            shortest = delays.min() ?? 0.1
        }

        func image(at time: Double) -> CGImage {
            let t = time.truncatingRemainder(dividingBy: total)
            let i = ends.firstIndex { t < $0 } ?? (images.count - 1)
            return images[i]
        }

        /// Reads the frames at a cover's size (at most 1400 pixels across), with their delays.
        static func load(_ url: URL) -> Frames? {
            guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
            let count = min(CGImageSourceGetCount(source), 600)
            let options: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1400,
            ]
            var images: [CGImage] = []
            var delays: [Double] = []
            for i in 0..<count {
                guard let image = CGImageSourceCreateThumbnailAtIndex(source, i, options as CFDictionary) else { continue }
                images.append(image)
                delays.append(delay(source, i))
            }
            return images.count > 1 ? Frames(images: images, delays: delays) : nil
        }

        /// A frame's delay, as browsers read it: under 0.02 s counts as 0.1 s.
        private static func delay(_ source: CGImageSource, _ index: Int) -> Double {
            let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any] ?? [:]
            let found: Double? = {
                if let webp = properties[kCGImagePropertyWebPDictionary] as? [CFString: Any] {
                    return (webp[kCGImagePropertyWebPUnclampedDelayTime] ?? webp[kCGImagePropertyWebPDelayTime]) as? Double
                }
                if let gif = properties[kCGImagePropertyGIFDictionary] as? [CFString: Any] {
                    return (gif[kCGImagePropertyGIFUnclampedDelayTime] ?? gif[kCGImagePropertyGIFDelayTime]) as? Double
                }
                return nil
            }()
            guard let found, found >= 0.02 else { return 0.1 }
            return found
        }
    }
}

/// A moving cover over a still one (the player's square or record cover): the song's video or
/// animated picture, filling its frame, while `on`. Nothing for a song on the computer.
struct MovingCoverLayer: View {
    @Environment(Store.self) private var store
    let song: Song?
    let on: Bool
    @State private var file: URL?

    var body: some View {
        ZStack {
            if on, let file {
                if AnimatedImage.isPicture(file) { AnimatedImage(url: file) } else { LoopingVideo(url: file) }
            }
        }
        .allowsHitTesting(false)
        .task(id: "\(song?.id ?? "")|\(on)") {
            file = nil
            guard on, let id = song?.id, !(song?.onComputer ?? true) else { return }
            let found = (await store.load { $0.animatedCover(id: id) } ?? nil).map(URL.init(fileURLWithPath:))
            if !Task.isCancelled { file = found }
        }
    }
}
