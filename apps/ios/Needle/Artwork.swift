import CoreImage
import ImageIO
import SwiftUI
import UIKit

/// Goes up when an artist's own picture is chosen or reset: the picture views read their files
/// again then (the path stays the same, so nothing else tells them).
@Observable
final class PictureChanges {
    nonisolated(unsafe) static let shared = PictureChanges()
    var count = 0
}

/// Covers, read from their files away from the screen, made small for the size they show at,
/// and kept for a while.
enum Covers {
    nonisolated(unsafe) private static let cache: NSCache<NSString, UIImage> = {
        let cache = NSCache<NSString, UIImage>()
        cache.totalCostLimit = 96 * 1024 * 1024
        return cache
    }()

    /// The picture's path with its file's change time. An artist's own photo, wide picture,
    /// and logo are saved again at the same path when another is chosen: with the time in the
    /// key, the new one is read, never the old one from a cache.
    nonisolated static func stamp(_ path: String) -> String {
        guard !path.hasPrefix("http://"), !path.hasPrefix("https://"),
              let date = (try? FileManager.default.attributesOfItem(atPath: path))?[.modificationDate] as? Date else { return path }
        return "\(path)#\(date.timeIntervalSince1970)"
    }

    nonisolated static func stamp(_ path: String?) -> String? { path.map { stamp($0) } }

    /// Whether a picture is dark where it is drawn: its alpha-weighted lightness under 0.4, as
    /// on Windows (fanart.tv has black logos beside the white ones).
    nonisolated static func isDark(_ cg: CGImage) -> Bool {
        let side = 64
        guard let context = CGContext(data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return false }
        context.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        guard let data = context.data else { return false }
        let p = data.bindMemory(to: UInt8.self, capacity: side * side * 4)
        var (light, weight) = (0.0, 0.0)
        for i in 0..<(side * side) {
            // Kept premultiplied: each colour is already weighted by its alpha.
            let (r, g, b, a) = (Double(p[i * 4]), Double(p[i * 4 + 1]), Double(p[i * 4 + 2]), Double(p[i * 4 + 3]))
            light += (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255
            weight += a / 255
        }
        return weight > 0 && light / weight < 0.4
    }

    static func cached(_ path: String, size: Int) -> UIImage? {
        cache.object(forKey: "\(size)@\(stamp(path))" as NSString)
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
            cache.setObject(image, forKey: "\(size)@\(stamp(path))" as NSString, cost: cg.bytesPerRow * cg.height)
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
        // The space the page gives sets the size, never the picture: a cover that is not square
        // (a Blu-ray's, say) fills it, its edges cut off, instead of making its tile taller.
        Color.clear
            .overlay {
                if let image {
                    Image(uiImage: image).resizable().scaledToFill()
                } else {
                    Placeholder(seed: seed)
                }
            }
            .clipped()
        .task(id: "\(Covers.stamp(path) ?? "")|\(PictureChanges.shared.count)") {
            guard let path else { image = nil; return }
            if let hit = Covers.cached(path, size: size) { image = hit; return }
            let found = await Covers.load(path, size: size)
            // Moved on to another cover while this one loaded: not this one.
            if !Task.isCancelled { image = found }
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
    private static let lock = NSLock()

    private static func kept(_ key: String) -> PageColors? {
        lock.withLock { known[key] }
    }

    /// Keeps a page's colours under a name of its own (an artist's, say), for its next visit.
    static func keep(_ colors: PageColors, as key: String) {
        lock.withLock { known[key] = colors }
    }

    /// Already found for this picture (or made-up cover), or kept under `key`, without waiting.
    static func cached(_ path: String?, seed: String) -> PageColors? {
        kept(Covers.stamp(path) ?? "seed:" + seed)
    }

    static func cached(key: String) -> PageColors? {
        kept(key)
    }

    /// For the picture at `path`, or the made-up cover of `seed` when there is none.
    static func of(_ path: String?, seed: String) async -> PageColors {
        let key = Covers.stamp(path) ?? "seed:" + seed
        if let hit = kept(key) { return hit }
        var rgb: (Double, Double, Double)?
        if let path, let image = await Covers.load(path, size: 64), let cg = image.cgImage {
            rgb = foot(of: cg)
        }
        // The picture could not be read (gone, or on a computer that is off): the made-up
        // cover's colours for now, not kept, so the picture's come once it can be read.
        let failed = path != nil && rgb == nil
        if rgb == nil {
            // The made-up cover runs from its first colour into near black.
            var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            UIColor(Palette(seed).colors[0]).getRed(&r, green: &g, blue: &b, alpha: &a)
            rgb = (Double(r) * 0.35, Double(g) * 0.35, Double(b) * 0.35)
        }
        let colors = make(rgb!)
        if !failed { lock.withLock { known[key] = colors } }
        return colors
    }

    /// The average colour of the picture's lowest rows.
    private static func foot(of cg: CGImage) -> (Double, Double, Double)? {
        let side = 32
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        // The pixels' memory stays put only inside this call.
        let drawn = pixels.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                          space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
            return true
        }
        guard drawn else { return nil }
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
        // WCAG's contrast: white on the page against near-black on it.
        func linear(_ c: CGFloat) -> CGFloat { c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4) }
        let lum = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        let black = linear(0.06)
        let light = (lum + 0.05) / (black + 0.05) > 1.05 / (lum + 0.05)
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
    /// Drawn in this colour, its own colours left out: for a light page.
    var ink: Color?
    @State private var image: UIImage?
    /// A dark logo: drawn in white on a dark page, where it would not show.
    @State private var dark = false

    /// Chosen by the listener (a file, or a fanart.tv pick): kept as it is.
    private var chosen: Bool { path.hasSuffix(".own") }

    var body: some View {
        Group {
            if let image {
                // A logo the listener chose (the core keeps it as ".own") shows as it is: their
                // pick of a white or a black logo stays. Needle recolours only the ones it found.
                if let ink, !chosen {
                    Image(uiImage: image).renderingMode(.template).resizable().scaledToFit().foregroundStyle(ink)
                } else if dark, !chosen {
                    Image(uiImage: image).renderingMode(.template).resizable().scaledToFit().foregroundStyle(Color(white: 245 / 255))
                } else {
                    Image(uiImage: image).resizable().scaledToFit()
                }
            } else {
                Color.clear
            }
        }
        .task(id: "\(Covers.stamp(path))|\(PictureChanges.shared.count)") {
            let found = await Covers.load(path, size: 900)
            let cg = found?.cgImage
            let isDark = cg.map(Covers.isDark) ?? false
            if !Task.isCancelled { image = found; dark = isDark }
        }
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
/// The cover is blurred once into a small picture, which then only turns: blurring a picture
/// larger than the screen each frame used the battery.
struct CoverBackdrop: View {
    let path: String?
    let seed: String
    var blur: CGFloat = 70
    var moving = true
    @State private var soft: UIImage?

    init(path: String?, seed: String, blur: CGFloat = 70, moving: Bool = true) {
        self.path = path
        self.seed = seed
        self.blur = blur
        self.moving = moving
        // The one made before, so a new backdrop (the lyrics opened) never starts black.
        _soft = State(initialValue: Self.made.object(forKey: (Covers.stamp(path) ?? seed) as NSString)?.image)
    }

    /// The blurred picture's side, in pixels.
    private static let pixels: CGFloat = 160

    /// A blurred picture and how soft it was made.
    private final class Made {
        let image: UIImage
        let radius: Int
        init(_ image: UIImage, _ radius: Int) { self.image = image; self.radius = radius }
    }

    nonisolated(unsafe) private static let made: NSCache<NSString, Made> = {
        let cache = NSCache<NSString, Made>()
        cache.countLimit = 12
        return cache
    }()
    nonisolated(unsafe) private static let context = CIContext()

    var body: some View {
        GeometryReader { geo in
            let side = max(geo.size.width, geo.size.height) * 1.6
            // The blur, in the small picture's pixels: as soft as `blur` points at full size.
            let radius = max(1, blur * Self.pixels / max(1, side))
            RecordSpin(on: moving, period: 60, rate: 15) {
                Group {
                    if let soft { Image(uiImage: soft).resizable().interpolation(.medium) } else { Color.black }
                }
                .frame(width: side, height: side)
            }
            .position(x: geo.size.width / 2, y: geo.size.height / 2)
            .task(id: "\(Covers.stamp(path) ?? seed)|\(Int(radius))|\(PictureChanges.shared.count)") {
                let key = (Covers.stamp(path) ?? seed) as NSString
                if let kept = Self.made.object(forKey: key), kept.radius == Int(radius) {
                    soft = kept.image
                    return
                }
                // A cover that cannot be read (gone, or on a computer that is off): the made-up one.
                var source: UIImage? = if let path { await Covers.load(path, size: Int(Self.pixels)) } else { nil }
                // Kept only when it is the cover itself (or the made-up one asked for), so the
                // cover shows once it can be read.
                let real = source != nil || path == nil
                if source == nil { source = placeholder }
                guard !Task.isCancelled, let source else { return }
                let made = await Task.detached(priority: .userInitiated) { Self.soften(source, radius: radius) }.value
                guard !Task.isCancelled, let made else { return }
                if real { Self.made.setObject(Made(made, Int(radius)), forKey: key) }
                withAnimation(.easeInOut(duration: 0.4)) { soft = made }
            }
        }
        .overlay(LinearGradient(colors: [.black.opacity(0.05), .black.opacity(0.5)], startPoint: .top, endPoint: .bottom))
        .ignoresSafeArea()
    }

    /// The made-up cover, as a small picture.
    @MainActor private var placeholder: UIImage? {
        let renderer = ImageRenderer(content: Placeholder(seed: seed).frame(width: Self.pixels, height: Self.pixels))
        renderer.scale = 1
        return renderer.uiImage
    }

    /// Blurred, richer, and a little darker, as the player's backdrop.
    nonisolated private static func soften(_ image: UIImage, radius: CGFloat) -> UIImage? {
        guard let cg = image.cgImage else { return nil }
        let input = CIImage(cgImage: cg)
        let blurred = input.clampedToExtent()
            .applyingGaussianBlur(sigma: Double(radius))
            .applyingFilter("CIColorControls", parameters: [kCIInputSaturationKey: 1.7, kCIInputBrightnessKey: -0.18])
            .cropped(to: input.extent)
        guard let out = context.createCGImage(blurred, from: input.extent) else { return nil }
        return UIImage(cgImage: out)
    }
}
