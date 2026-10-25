import SwiftUI

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
                            .clipShape(.rect(cornerRadius: 6))
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

/// A line chosen in the lyrics (held, then Share as Card).
struct CardLine: Identifiable {
    let id = UUID()
    let text: String
}

/// The card for a chosen line, and Share (Save Image is in the share sheet).
struct LyricCardSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    let line: String
    let song: Song
    @State private var cover: CGImage?
    @State private var picture: UIImage?

    var body: some View {
        NavigationStack {
            VStack(spacing: 22) {
                card
                    .scaleEffect(0.86)
                    .frame(width: 310, height: 310)
                    .clipShape(.rect(cornerRadius: 18))
                    .shadow(color: .black.opacity(0.3), radius: 20, y: 10)
                    .accessibilityLabel("A card of the line: \(line)")
                if let picture {
                    let image = Image(uiImage: picture)
                    ShareLink(item: image, preview: SharePreview(line, image: image)) {
                        Label("Share", systemImage: "square.and.arrow.up").frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    .foregroundStyle(store.colors.onAccent)
                } else {
                    ProgressView()
                }
            }
            .padding(24)
            .navigationTitle("Lyric card")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
        .task {
            // The cover first, read whole, so the picture is made with it.
            if let path = song.artwork { cover = await Covers.load(path, size: 1080)?.cgImage }
            let renderer = ImageRenderer(content: card)
            renderer.scale = 3
            picture = renderer.uiImage
        }
    }

    private var card: LyricCard {
        LyricCard(line: line, title: song.title, artist: song.artist, cover: cover, tint: store.coverColor ?? store.colors.accent)
    }
}
