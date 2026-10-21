import SwiftUI

/// The first start: what Needle is, and where the music can come from.
struct WelcomeView: View {
    @Environment(Store.self) private var store
    let open: (Place, Route?) -> Void
    @State private var importing = false

    var body: some View {
        ZStack {
            CoverBackdrop(path: nil, seed: "Needle", blur: 90, moving: true)
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Image(uiImage: UIImage(named: "AppIcon") ?? UIImage())
                        .resizable().frame(width: 76, height: 76).clipShape(.rect(cornerRadius: 18))
                        .padding(.top, 60)
                    Text("Welcome to Needle").font(.largeTitle.bold())
                    Text("Your music, from this iPhone, your music server, and Needle on your computer. No account, and nothing leaves your phone unless you ask.")
                        .font(.title3).foregroundStyle(.secondary)
                    VStack(spacing: 12) {
                        choice("folder.badge.plus", "Import from Files", "Songs on this iPhone or in iCloud Drive.") { importing = true }
                        choice("server.rack", "Connect a music server", "Navidrome or Subsonic.") { finish(.sources, .plugins) }
                        choice("laptopcomputer", "Connect your computer", "Play its music here, and move what plays.") { finish(.sources, .computer) }
                    }
                    .padding(.top, 8)
                    Button("Look around first") { finish(.home, nil) }
                        .frame(maxWidth: .infinity)
                        .padding(.top, 8)
                }
                .padding(24)
            }
        }
        .preferredColorScheme(.dark)
        .fileImporter(isPresented: $importing, allowedContentTypes: [.audio, .folder], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { store.importFiles(urls) }
            finish(.home, nil)
        }
    }

    private func choice(_ symbol: String, _ title: String, _ detail: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.title2).foregroundStyle(.tint).frame(width: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline)
                    Text(detail).font(.subheadline).foregroundStyle(.secondary)
                }
                Spacer()
                Image(systemName: "chevron.right").foregroundStyle(.tertiary)
            }
            .padding(16)
            .frame(maxWidth: .infinity)
            .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 22))
        }
        .buttonStyle(.plain)
    }

    private func finish(_ place: Place, _ route: Route?) {
        store.updateApp { $0.welcomed = true }
        open(place, route)
    }
}
