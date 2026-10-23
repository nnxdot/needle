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
            if let frames {
                TimelineView(.animation(minimumInterval: frames.shortest)) { context in
                    if let image = frames.image(at: context.date.timeIntervalSinceReferenceDate) {
                        Image(decorative: image, scale: 1)
                            .resizable()
                            .scaledToFill()
                    }
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
            let line = "\(url.lastPathComponent): \(frames?.count ?? 0) frames, \(String(format: "%.2f", frames?.total ?? 0)) s a loop\n"
            try? line.write(toFile: NSTemporaryDirectory() + "needle-animated.log", atomically: true, encoding: .utf8)
            #endif
        }
        .accessibilityHidden(true)
    }

    /// The frames, each with the moment it ends within one loop. Small animations are read
    /// once and kept; large ones (an album's 300-frame WebP is gigabytes as pictures) are read
    /// a frame at a time, as they come up, with only a few kept.
    final class Frames: @unchecked Sendable {
        let count: Int
        let ends: [Double]
        let total: Double
        let shortest: Double
        private let source: CGImageSource
        private let options: CFDictionary
        private let lock = NSLock()
        private var kept: [Int: CGImage]
        /// All frames are in `kept` (a small animation).
        private let whole: Bool
        private var reading = Set<Int>()
        private var last: CGImage?
        private static let queue = DispatchQueue(label: "needle.animated-frames", qos: .userInitiated)

        private init(source: CGImageSource, options: CFDictionary, delays: [Double], kept: [Int: CGImage], whole: Bool) {
            self.source = source
            self.options = options
            self.count = delays.count
            var at = 0.0
            ends = delays.map { at += $0; return at }
            total = max(at, 0.01)
            shortest = delays.min() ?? 0.1
            self.kept = kept
            self.whole = whole
            last = kept[0]
        }

        func index(at time: Double) -> Int {
            let t = time.truncatingRemainder(dividingBy: total)
            return ends.firstIndex { t < $0 } ?? (count - 1)
        }

        /// The frame for this moment, or the last one shown while it is still being read.
        func image(at time: Double) -> CGImage? {
            let i = index(at: time)
            lock.lock()
            defer { lock.unlock() }
            if let image = kept[i] {
                last = image
                if !whole { read((i + 1) % count) }
                return image
            }
            read(i)
            read((i + 1) % count)
            return last
        }

        /// Reads a frame in the background (call with the lock held); keeps the frames around it.
        private func read(_ i: Int) {
            guard !whole, kept[i] == nil, !reading.contains(i) else { return }
            reading.insert(i)
            Self.queue.async { [self] in
                let image = CGImageSourceCreateThumbnailAtIndex(source, i, options)
                lock.lock()
                reading.remove(i)
                if let image { kept[i] = image }
                // Only the frames near this one stay.
                for key in kept.keys where (key - i + count) % count > 2 && (i - key + count) % count > 1 {
                    kept[key] = nil
                }
                lock.unlock()
            }
        }

        /// Reads the delays (and, for a small animation, every frame) at a cover's size.
        static func load(_ url: URL) -> Frames? {
            guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
            let count = min(CGImageSourceGetCount(source), 2000)
            guard count > 1 else { return nil }
            let options = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1400,
            ] as CFDictionary
            let delays = (0..<count).map { delay(source, $0) }
            // The size of every frame at most 1400 across, against a budget of about 160 MB.
            var side = 1400.0
            if let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
               let w = properties[kCGImagePropertyPixelWidth] as? Double, let h = properties[kCGImagePropertyPixelHeight] as? Double {
                side = min(max(w, h), 1400)
            }
            let whole = Double(count) * side * side * 4 <= 160_000_000
            var kept: [Int: CGImage] = [:]
            if whole {
                for i in 0..<count {
                    if let image = CGImageSourceCreateThumbnailAtIndex(source, i, options) { kept[i] = image }
                }
                guard kept.count > 1 else { return nil }
            } else if let first = CGImageSourceCreateThumbnailAtIndex(source, 0, options) {
                kept[0] = first
            } else {
                return nil
            }
            return Frames(source: source, options: options, delays: delays, kept: kept, whole: whole)
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
            let found = await store.movingCover(id)
            if !Task.isCancelled { file = found }
        }
    }
}
