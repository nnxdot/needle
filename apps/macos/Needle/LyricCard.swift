import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// A lyric line as a square picture to share: the line large over the blurred cover in its
/// colour, with the song, its artist, and the cover at the top.
struct LyricCard: View {
    let line: String
    let title: String
    let artist: String
    let cover: CGImage?
    let tint: Color

    var body: some View {
        ZStack(alignment: .topLeading) {
            Group {
                if let cover {
                    Image(decorative: cover, scale: 1).resizable().scaledToFill().blur(radius: 40).scaleEffect(1.3)
                } else {
                    tint
                }
            }
            .frame(width: 360, height: 360)
            .clipped()
            LinearGradient(colors: [tint.opacity(0.55), .black.opacity(0.7)], startPoint: .topLeading, endPoint: .bottomTrailing)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 10) {
                    if let cover {
                        Image(decorative: cover, scale: 1).resizable().scaledToFill()
                            .frame(width: 44, height: 44)
                            .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                            .shadow(color: .black.opacity(0.3), radius: 6, y: 3)
                    }
                    VStack(alignment: .leading, spacing: 1) {
                        Text(title).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                        Text(artist).font(.system(size: 12)).opacity(0.75).lineLimit(1)
                    }
                }
                Spacer(minLength: 16)
                Text(line)
                    .font(.system(size: line.count < 40 ? 34 : line.count < 90 ? 28 : 22, weight: .heavy))
                    .lineLimit(7)
                    .minimumScaleFactor(0.5)
                Spacer(minLength: 16)
                HStack {
                    Spacer()
                    Text("Needle").font(.system(size: 11, weight: .bold)).kerning(0.5).opacity(0.6)
                }
            }
            .foregroundStyle(.white)
            .padding(26)
        }
        .frame(width: 360, height: 360)
        .environment(\.colorScheme, .dark)
    }
}

/// A line chosen in the lyrics (right-click, Share as Card…).
struct CardLine: Identifiable {
    let id = UUID()
    let text: String
}

/// The card for a chosen line: Share, Copy, and Save….
struct LyricCardSheet: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let line: String
    @State private var cover: CGImage?
    @State private var picture: NSImage?

    var body: some View {
        VStack(spacing: 18) {
            Text("Lyric card").font(.headline)
            card
                .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                .shadow(color: .black.opacity(0.3), radius: 18, y: 8)
                .accessibilityLabel("A card of the line: \(line)")
            HStack {
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
                Spacer()
                if let picture {
                    Button("Copy") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.writeObjects([picture])
                        store.notify("Copied the card")
                    }
                    Button("Save…") { save(picture) }
                    let image = Image(nsImage: picture)
                    ShareLink(item: image, preview: SharePreview(line, image: image)) {
                        Label("Share", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                }
            }
        }
        .padding(24)
        .frame(width: 420)
        .task {
            // The cover first, read whole, so the picture is made with it.
            if let path = store.playback?.artwork,
               let image = await Covers.load(path, size: 1080) {
                cover = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
            }
            let renderer = ImageRenderer(content: card)
            renderer.scale = 3
            picture = renderer.nsImage
        }
    }

    private var card: LyricCard {
        let p = store.playback
        return LyricCard(line: line, title: p?.title ?? "", artist: p?.artist ?? "", cover: cover,
                         tint: store.coverTint.map(Color.init(nsColor:)) ?? store.colors.accent)
    }

    private func save(_ picture: NSImage) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.png]
        panel.nameFieldStringValue = "\(store.playback?.title ?? "Lyrics") — lyric card.png"
        guard panel.runModal() == .OK, let url = panel.url,
              let tiff = picture.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
              let png = rep.representation(using: .png, properties: [:]) else { return }
        try? png.write(to: url)
    }
}
