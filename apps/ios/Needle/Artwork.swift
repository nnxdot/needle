import ImageIO
import SwiftUI
import UIKit

/// Covers, read from their files away from the screen, made small for the size they show at,
/// and kept for a while.
enum Covers {
    nonisolated(unsafe) private static let cache: NSCache<NSString, UIImage> = {
        let cache = NSCache<NSString, UIImage>()
        cache.totalCostLimit = 96 * 1024 * 1024
        return cache
    }()

    static func cached(_ path: String, size: Int) -> UIImage? {
        cache.object(forKey: "\(size)@\(path)" as NSString)
    }

    static func load(_ path: String, size: Int) async -> UIImage? {
        if let hit = cached(path, size: size) { return hit }
        // Covers of songs on the computer come over the home network.
        var data: Data?
        if path.hasPrefix("http://") || path.hasPrefix("https://") {
            guard let url = URL(string: path) else { return nil }
            data = try? await URLSession.shared.data(from: url).0
            if data == nil { return nil }
        }
        return await Task.detached(priority: .userInitiated) { [data] in
            let source: CGImageSource? = if let data {
                CGImageSourceCreateWithData(data as CFData, nil)
            } else {
                CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil)
            }
            guard let source else { return nil }
            let options: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: size,
            ]
            guard let cg = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
            let image = UIImage(cgImage: cg)
            cache.setObject(image, forKey: "\(size)@\(path)" as NSString, cost: cg.bytesPerRow * cg.height)
            return image
        }.value
    }
}

/// A cover, or, when there is none, a picture made from the name: colours and a shape that
/// stay the same for the same album.
struct Artwork: View {
    let path: String?
    let seed: String
    var size: Int = 600
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                Placeholder(seed: seed)
            }
        }
        .clipped()
        .task(id: path) {
            guard let path else { image = nil; return }
            if let hit = Covers.cached(path, size: size) { image = hit; return }
            image = await Covers.load(path, size: size)
        }
    }
}

/// The colours of a name: three, from its hash, always the same for the same name.
struct Palette {
    let colors: [Color]
    let pattern: Int

    init(_ seed: String) {
        var hash: UInt64 = 1469598103934665603
        for byte in seed.utf8 { hash = (hash ^ UInt64(byte)) &* 1099511628211 }
        let base = Double(hash % 360) / 360
        colors = [0.0, 0.09, 0.5].enumerated().map { i, shift in
            Color(hue: (base + shift).truncatingRemainder(dividingBy: 1),
                  saturation: [0.75, 0.6, 0.35][i], brightness: [0.85, 0.95, 1][i])
        }
        pattern = Int((hash >> 20) % 4)
    }
}

struct Placeholder: View {
    let seed: String

    var body: some View {
        let p = Palette(seed)
        GeometryReader { geo in
            let s = min(geo.size.width, geo.size.height)
            ZStack {
                LinearGradient(colors: [p.colors[0], .black.opacity(0.85)], startPoint: .topLeading, endPoint: .bottomTrailing)
                switch p.pattern {
                case 0:
                    RadialGradient(colors: [p.colors[1], .clear], center: .bottomLeading, startRadius: 0, endRadius: s * 0.9)
                    Circle().fill(p.colors[2]).frame(width: s * 0.32).offset(x: s * 0.18, y: -s * 0.18)
                case 1:
                    RadialGradient(colors: [p.colors[1], .clear], center: UnitPoint(x: 0.3, y: 0.7), startRadius: 0, endRadius: s * 0.6)
                    ForEach(0..<7) { i in
                        Rectangle().fill(p.colors[2].opacity(0.22)).frame(width: s * 0.06, height: s * 2)
                            .rotationEffect(.degrees(25)).offset(x: CGFloat(i - 3) * s * 0.16)
                    }
                case 2:
                    AngularGradient(colors: [p.colors[0], p.colors[1], p.colors[2], p.colors[0]], center: UnitPoint(x: 0.5, y: 0.55), angle: .degrees(210))
                    RadialGradient(colors: [.white.opacity(0.33), .clear], center: UnitPoint(x: 0.35, y: 0.3), startRadius: 0, endRadius: s * 0.4)
                default:
                    ForEach(0..<4) { i in
                        Circle().strokeBorder(i % 2 == 0 ? p.colors[2] : p.colors[1], lineWidth: s * 0.05)
                            .frame(width: s * (0.5 + CGFloat(i) * 0.3))
                            .offset(y: s * 0.62)
                    }
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
        }
    }
}

/// The playing cover, blurred and large, behind the full player; turning slowly when `moving`.
struct CoverBackdrop: View {
    let path: String?
    let seed: String
    var blur: CGFloat = 70
    var moving = true

    var body: some View {
        GeometryReader { geo in
            let side = max(geo.size.width, geo.size.height) * 1.6
            RecordSpin(on: moving, period: 60) {
                Artwork(path: path, seed: seed, size: 200)
                    .frame(width: side, height: side)
            }
            .blur(radius: blur)
            .saturation(1.7)
            .brightness(-0.18)
            .position(x: geo.size.width / 2, y: geo.size.height / 2)
        }
        .overlay(LinearGradient(colors: [.black.opacity(0.05), .black.opacity(0.5)], startPoint: .top, endPoint: .bottom))
        .ignoresSafeArea()
    }
}
