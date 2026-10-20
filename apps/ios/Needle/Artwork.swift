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

/// A page's colours from its picture, as Apple Music's album and artist pages take them: the
/// colour along the picture's foot fills the page under it, so the picture fades into the page,
/// and the text is white or black, whichever reads on it.
struct PageColors: Equatable {
    var page: Color
    var ink: Color
    var dark: Bool

    var muted: Color { ink.opacity(0.62) }
    var line: Color { ink.opacity(0.14) }

    /// The theme's own page, for when colours from covers are off.
    static func plain(_ theme: ThemeColors) -> PageColors {
        PageColors(page: theme.page, ink: theme.text, dark: theme.dark)
    }

    nonisolated(unsafe) private static var known: [String: PageColors] = [:]

    /// Already found for this picture (or made-up cover), without waiting.
    static func cached(_ path: String?, seed: String) -> PageColors? {
        known[path ?? "seed:" + seed]
    }

    /// For the picture at `path`, or the made-up cover of `seed` when there is none.
    static func of(_ path: String?, seed: String) async -> PageColors {
        let key = path ?? "seed:" + seed
        if let hit = known[key] { return hit }
        var rgb: (Double, Double, Double)?
        if let path, let image = await Covers.load(path, size: 64), let cg = image.cgImage {
            rgb = foot(of: cg)
        }
        if rgb == nil {
            // The made-up cover runs from its first colour into near black.
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            UIColor(Palette(seed).colors[0]).getRed(&r, green: &g, blue: &b, alpha: &a)
            rgb = (Double(r) * 0.35, Double(g) * 0.35, Double(b) * 0.35)
        }
        let colors = make(rgb!)
        known[key] = colors
        return colors
    }

    /// The average colour of the picture's lowest rows.
    private static func foot(of cg: CGImage) -> (Double, Double, Double)? {
        let side = 32
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        guard let context = CGContext(data: &pixels, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        // The bitmap's first row is the picture's top.
        var sum = (0.0, 0.0, 0.0), n = 0.0
        for y in (side - 5)..<side {
            for x in 0..<side {
                let i = (y * side + x) * 4
                sum.0 += Double(pixels[i]); sum.1 += Double(pixels[i + 1]); sum.2 += Double(pixels[i + 2]); n += 1
            }
        }
        return (sum.0 / n / 255, sum.1 / n / 255, sum.2 / n / 255)
    }

    /// A little richer than the picture's own colour, never quite white or black.
    private static func make(_ rgb: (Double, Double, Double)) -> PageColors {
        var h: CGFloat = 0, s: CGFloat = 0, v: CGFloat = 0, a: CGFloat = 0
        UIColor(red: rgb.0, green: rgb.1, blue: rgb.2, alpha: 1).getHue(&h, saturation: &s, brightness: &v, alpha: &a)
        let page = UIColor(hue: h, saturation: min(1, s * 1.12), brightness: min(0.9, max(0.1, v)), alpha: 1)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0
        page.getRed(&r, green: &g, blue: &b, alpha: &a)
        let light = 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.62
        return PageColors(page: Color(page), ink: light ? Color(white: 0.06) : .white, dark: !light)
    }
}

extension Color {
    /// The same colour, no brighter than `brightness` (0 to 1).
    func darkened(to brightness: CGFloat) -> Color {
        var h: CGFloat = 0, s: CGFloat = 0, v: CGFloat = 0, a: CGFloat = 0
        UIColor(self).getHue(&h, saturation: &s, brightness: &v, alpha: &a)
        return Color(UIColor(hue: h, saturation: s, brightness: min(v, brightness), alpha: 1))
    }
}

extension View {
    /// A page in its picture's colours: text, tint, and the system's controls follow them.
    func paged(_ colors: PageColors) -> some View {
        self
            .scrollContentBackground(.hidden)
            .background(colors.page.ignoresSafeArea())
            .foregroundStyle(colors.ink)
            .tint(colors.ink)
            .environment(\.colorScheme, colors.dark ? .dark : .light)
            .toolbarColorScheme(colors.dark ? .dark : .light, for: .navigationBar)
            .animation(.easeOut(duration: 0.35), value: colors)
    }
}

/// An artist's logo: a picture with a clear background, kept whole.
struct Logo: View {
    let path: String
    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image { Image(uiImage: image).resizable().scaledToFit() } else { Color.clear }
        }
        .task(id: path) { image = await Covers.load(path, size: 900) }
    }
}

/// A picture that grows when the page is pulled down past its top, as headers do in Music.
struct Stretchy: ViewModifier {
    func body(content: Content) -> some View {
        content.visualEffect { content, proxy in
            let pulled = max(0, proxy.frame(in: .scrollView).minY)
            return content.scaleEffect(1 + pulled / max(1, proxy.size.height), anchor: .bottom)
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
