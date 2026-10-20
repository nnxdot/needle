import AppKit
import CoreImage
import ImageIO
import SwiftUI

/// Covers, read from their files away from the window, made small for the size they show at,
/// and kept for a while.
enum Covers {
    nonisolated(unsafe) private static let cache: NSCache<NSString, NSImage> = {
        let cache = NSCache<NSString, NSImage>()
        cache.totalCostLimit = 160 * 1024 * 1024
        return cache
    }()

    static func cached(_ path: String, size: Int) -> NSImage? {
        cache.object(forKey: "\(size)@\(path)" as NSString)
    }

    nonisolated static func cgImage(_ path: String, size: Int) -> CGImage? {
        let source: CGImageSource?
        if path.hasPrefix("http://") || path.hasPrefix("https://") {
            guard let url = URL(string: path), let data = try? Data(contentsOf: url) else { return nil }
            source = CGImageSourceCreateWithData(data as CFData, nil)
        } else {
            source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil)
        }
        guard let source else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: size,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }

    static func load(_ path: String, size: Int) async -> NSImage? {
        if let hit = cached(path, size: size) { return hit }
        return await Task.detached(priority: .userInitiated) {
            guard let cg = cgImage(path, size: size) else { return nil }
            let image = NSImage(cgImage: cg, size: NSSize(width: cg.width, height: cg.height))
            cache.setObject(image, forKey: "\(size)@\(path)" as NSString, cost: cg.bytesPerRow * cg.height)
            return image
        }.value
    }

    /// The cover blurred and made more colourful, small, for backgrounds: drawn large, a small
    /// blurred picture looks like a large one blurred, and costs nothing to draw again.
    static func backdrop(_ path: String) async -> NSImage? {
        let key = "backdrop@\(path)" as NSString
        if let hit = cache.object(forKey: key) { return hit }
        return await Task.detached(priority: .utility) {
            guard let cg = cgImage(path, size: 96) else { return nil }
            let source = CIImage(cgImage: cg)
            let soft = source.clampedToExtent()
                .applyingGaussianBlur(sigma: 7)
                .applyingFilter("CIColorControls", parameters: [kCIInputSaturationKey: 1.6])
                .cropped(to: source.extent)
            guard let out = CIContext().createCGImage(soft, from: source.extent) else { return nil }
            let image = NSImage(cgImage: out, size: NSSize(width: out.width, height: out.height))
            cache.setObject(image, forKey: key, cost: out.bytesPerRow * out.height)
            return image
        }.value
    }

    /// A page's colour from its picture, as Apple Music's album and artist pages take it: the
    /// colour along the picture's foot, a little richer, never quite white or black; for no
    /// picture, the made-up cover's first colour, darkened as the made-up cover is.
    @MainActor
    static func pageColor(_ path: String?, seed: String) async -> NSColor {
        let key = path ?? "seed:" + seed
        if let hit = pageColors[key] { return hit }
        var foot: (Double, Double, Double)?
        if let path { foot = await Task.detached(priority: .userInitiated) { footColor(path) }.value }
        let rgb = foot ?? {
            let c = NSColor(SeedPalette(seed).colors[0]).usingColorSpace(.sRGB) ?? .gray
            return (Double(c.redComponent) * 0.35, Double(c.greenComponent) * 0.35, Double(c.blueComponent) * 0.35)
        }()
        var h: CGFloat = 0, s: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        NSColor(srgbRed: rgb.0, green: rgb.1, blue: rgb.2, alpha: 1).getHue(&h, saturation: &s, brightness: &b, alpha: &a)
        let color = NSColor(hue: h, saturation: min(1, s * 1.12), brightness: min(0.9, max(0.1, b)), alpha: 1)
        pageColors[key] = color
        return color
    }

    @MainActor private static var pageColors: [String: NSColor] = [:]

    /// The average colour of the picture's lowest rows.
    nonisolated static func footColor(_ path: String) -> (Double, Double, Double)? {
        guard let cg = cgImage(path, size: 64) else { return nil }
        let side = 32
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        guard let context = CGContext(data: &pixels, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
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

    /// The cover's most colourful tone, for tinting the app (`nil` for a grey cover).
    nonisolated static func vivid(_ path: String) -> NSColor? {
        guard let cg = cgImage(path, size: 48) else { return nil }
        let side = 16
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        guard let context = CGContext(data: &pixels, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                      space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.interpolationQuality = .medium
        context.draw(cg, in: CGRect(x: 0, y: 0, width: side, height: side))
        // Hues weighted by how colourful and how bright each pixel is.
        var x = 0.0, y = 0.0, weight = 0.0, saturation = 0.0, brightness = 0.0
        for i in stride(from: 0, to: pixels.count, by: 4) {
            let color = NSColor(red: CGFloat(pixels[i]) / 255, green: CGFloat(pixels[i + 1]) / 255, blue: CGFloat(pixels[i + 2]) / 255, alpha: 1)
            var h: CGFloat = 0, s: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
            color.getHue(&h, saturation: &s, brightness: &b, alpha: &a)
            let w = Double(s * s) * Double(min(1, b * 1.6)) * (b < 0.12 ? 0.1 : 1)
            x += cos(Double(h) * 2 * .pi) * w
            y += sin(Double(h) * 2 * .pi) * w
            weight += w
            saturation += Double(s) * w
            brightness += Double(b) * w
        }
        guard weight > 0.4 else { return nil }
        var hue = atan2(y, x) / (2 * .pi)
        if hue < 0 { hue += 1 }
        let s = saturation / weight, b = brightness / weight
        guard s > 0.14 else { return nil }
        return NSColor(hue: hue, saturation: min(1, s * 1.15), brightness: max(0.5, min(0.95, b)), alpha: 1)
    }
}

/// A cover, or, when there is none, a picture made from the name: colours and a shape that
/// stay the same for the same album.
struct Artwork: View {
    let path: String?
    let seed: String
    var size: Int = 600
    @State private var image: NSImage?

    var body: some View {
        ZStack {
            if let image {
                Image(nsImage: image).resizable().interpolation(.high).scaledToFill()
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

/// A square cover with rounded corners.
struct Cover: View {
    let path: String?
    let seed: String
    var side: CGFloat
    var corner: CGFloat? = nil

    var body: some View {
        Artwork(path: path, seed: seed, size: Int(side * 2))
            .frame(width: side, height: side)
            .clipShape(RoundedRectangle(cornerRadius: corner ?? max(3, side * 0.06), style: .continuous))
    }
}

/// The colours of a name: three, from its hash, always the same for the same name.
struct SeedPalette {
    let colors: [Color]
    let pattern: Int

    init(_ seed: String) {
        var hash: UInt64 = 1469598103934665603
        for byte in seed.utf8 { hash = (hash ^ UInt64(byte)) &* 1099511628211 }
        let base = Double(hash % 360) / 360
        colors = [0.0, 0.09, 0.5].enumerated().map { i, shift in
            Color(hue: (base + shift).truncatingRemainder(dividingBy: 1),
                  saturation: [0.65, 0.5, 0.3][i], brightness: [0.75, 0.88, 0.95][i])
        }
        pattern = Int((hash >> 20) % 4)
    }
}

struct Placeholder: View {
    let seed: String

    var body: some View {
        let p = SeedPalette(seed)
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

/// A round artist photo, or the artist's initials when no photo is known. Photos are found in
/// the library's folder, or online on the artist's own page when lookups are on.
struct ArtistPhoto: View {
    private var store: Store { .shared }
    let name: String
    var side: CGFloat
    /// May look online (only the artist's page does, so scrolling a grid floods nothing).
    var online = false
    @State private var path: String?

    var body: some View {
        ZStack {
            if let path {
                Artwork(path: path, seed: name, size: Int(side * 2))
            } else {
                let p = SeedPalette(name)
                LinearGradient(colors: [p.colors[0], p.colors[1].opacity(0.7)], startPoint: .topLeading, endPoint: .bottomTrailing)
                Text(initials).font(.system(size: side * 0.34, weight: .semibold, design: .rounded)).foregroundStyle(.white.opacity(0.92))
            }
        }
        .frame(width: side, height: side)
        .clipShape(Circle())
        .task(id: name) {
            let (name, online) = (name, online && store.settings.onlineMedia)
            path = await Photos.find(name, online: online, store: store)
        }
    }

    private var initials: String {
        name.split(separator: " ").filter { $0.lowercased() != "the" }.compactMap(\.first).prefix(2).map(String.init).joined().uppercased()
    }
}

/// Artist photos found so far, so a grid asks once per artist.
@MainActor
enum Photos {
    private static var found: [String: String?] = [:]

    static func find(_ name: String, online: Bool, store: Store) async -> String? {
        if let known = found[name], known != nil || !online { return known }
        let core = store.core
        // The photo already on this computer first; online only on the artist's page.
        let path = await Task.detached { () -> String? in
            online ? core.artistPhoto(name: name) : core.artistPhotoSaved(name: name)
        }.value
        if online || path != nil { found[name] = .some(path) }
        return path
    }
}

/// The top of an album's or artist's page: its picture blurred across the whole width,
/// fading into the page's colour, and under the sidebar's and the bar's glass as well
/// (macOS 26), as Apple Music's pages are.
struct HeaderBackdrop: View {
    private var store: Store { .shared }
    let path: String?
    let seed: String
    var height: CGFloat = 440
    @State private var image: NSImage?

    var body: some View {
        let canvas = store.colors.canvas
        // The page's width, whatever the picture's shape.
        Color.clear
            .frame(maxWidth: .infinity)
            .frame(height: height)
            .overlay {
                if let image {
                    Image(nsImage: image).resizable().interpolation(.medium).scaledToFill()
                } else {
                    Placeholder(seed: seed).blur(radius: 40).saturation(1.4)
                }
            }
            .clipped()
        .overlay {
            LinearGradient(stops: [
                .init(color: canvas.opacity(0.1), location: 0),
                .init(color: canvas.opacity(0.55), location: 0.55),
                .init(color: canvas, location: 1),
            ], startPoint: .top, endPoint: .bottom)
        }
        .modifier(ExtendsUnderGlass())
        .ignoresSafeArea(edges: .top)
        .allowsHitTesting(false)
        .task(id: path) {
            guard let path else { image = nil; return }
            image = await Covers.backdrop(path)
        }
    }
}

/// An artist's logo: a picture with a clear background, kept whole.
struct Logo: View {
    let path: String
    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image { Image(nsImage: image).resizable().interpolation(.high).scaledToFit() } else { Color.clear }
        }
        .task(id: path) { image = await Covers.load(path, size: 1200) }
    }
}

/// Under the sidebar and the bar, the picture goes on, mirrored and blurred (macOS 26).
struct ExtendsUnderGlass: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 26, *) { content.backgroundExtensionEffect() } else { content }
    }
}

/// A playlist's own picture, or a mosaic of four of its songs' covers.
struct PlaylistArt: View {
    let playlist: Playlist
    var side: CGFloat

    var body: some View {
        Group {
            if playlist.mosaic.count >= 4 {
                let half = side / 2
                VStack(spacing: 0) {
                    HStack(spacing: 0) {
                        Artwork(path: playlist.mosaic[0], seed: playlist.name, size: Int(side)).frame(width: half, height: half)
                        Artwork(path: playlist.mosaic[1], seed: playlist.name, size: Int(side)).frame(width: half, height: half)
                    }
                    HStack(spacing: 0) {
                        Artwork(path: playlist.mosaic[2], seed: playlist.name, size: Int(side)).frame(width: half, height: half)
                        Artwork(path: playlist.mosaic[3], seed: playlist.name, size: Int(side)).frame(width: half, height: half)
                    }
                }
            } else {
                Artwork(path: playlist.artwork, seed: playlist.name, size: Int(side * 2))
            }
        }
        .frame(width: side, height: side)
        .clipShape(RoundedRectangle(cornerRadius: max(3, side * 0.08), style: .continuous))
    }
}

/// The playing cover, blurred and large, behind the big player.
struct CoverBackdrop: View {
    let path: String?
    let seed: String
    var blur: CGFloat = 80
    var moving = true
    /// How dark the foot of it gets, for words over it.
    var dim: Double = 0.55
    /// For a light look: a light wash over the colours, for dark words, in place of darkening.
    var light = false
    @State private var image: NSImage?

    var body: some View {
        GeometryReader { geo in
            let side = max(geo.size.width, geo.size.height) * 1.5
            Group {
                if let image {
                    // Blurred once (see `Covers.backdrop`); only turned here.
                    TimelineView(.animation(minimumInterval: 1 / 15, paused: !moving)) { context in
                        let angle = moving ? context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 90) / 90 * 360 : 0
                        Image(nsImage: image)
                            .resizable()
                            .interpolation(.medium)
                            .frame(width: side, height: side)
                            .rotationEffect(.degrees(angle))
                    }
                } else {
                    // No cover file: the name's colours, blurred, held still.
                    Placeholder(seed: seed)
                        .frame(width: side, height: side)
                        .blur(radius: blur)
                        .saturation(1.6)
                }
            }
            .brightness(light ? 0.04 : dim > 0.4 ? -0.2 : -0.08)
            .position(x: geo.size.width / 2, y: geo.size.height / 2)
        }
        .overlay {
            if light {
                LinearGradient(colors: [.white.opacity(0.42), .white.opacity(0.3), .white.opacity(0.45)], startPoint: .top, endPoint: .bottom)
            } else {
                // Darker at the foot for words over it; with little dimming, a band at the top
                // too, so the title bar reads over a bright cover.
                LinearGradient(stops: dim > 0.4
                    ? [.init(color: .black.opacity(dim * 0.2), location: 0), .init(color: .black.opacity(dim), location: 1)]
                    : [.init(color: .black.opacity(0.3), location: 0), .init(color: .black.opacity(0.05), location: 0.18),
                       .init(color: .black.opacity(dim * 0.4), location: 0.5), .init(color: .black.opacity(dim), location: 1)],
                    startPoint: .top, endPoint: .bottom)
            }
        }
        .clipped()
        .task(id: path) {
            guard let path else { image = nil; return }
            image = await Covers.backdrop(path)
        }
    }
}
