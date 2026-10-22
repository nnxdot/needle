import SwiftUI

/// The first start: what Needle is, and where the music can come from.
struct WelcomeView: View {
    @Environment(Store.self) private var store
    let open: (Place, Route?) -> Void
    @State private var importing = false
    /// Off until chosen here: nothing leaves the phone unless asked.
    @State private var crashReports = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            CoverBackdrop(path: nil, seed: "Needle", blur: 90, moving: !reduceMotion && !store.app.reduceMotion)
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
                        choice("server.rack", "Connect a music server", "Navidrome or Subsonic.") { finish(.sources, .server("subsonic")) }
                        choice("laptopcomputer", "Connect your computer", "Play its music here, and move what plays.") { finish(.sources, .computer) }
                    }
                    .padding(.top, 8)
                    Button("Look around first") { finish(.home, nil) }
                        .frame(maxWidth: .infinity)
                        .padding(.top, 8)
                    Toggle(isOn: $crashReports) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Send crash reports").font(.subheadline.weight(.semibold))
                            Text("If Needle stops by mistake, it sends what went wrong (no music, names, or files).")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .padding(.top, 12)
                    .onChange(of: crashReports) { _, on in store.run { try $0.setCrashReports(on: on) } }
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
        let on = crashReports
        store.run { try $0.setCrashReports(on: on) }
        store.updateApp { $0.welcomed = true }
        open(place, route)
    }
}
