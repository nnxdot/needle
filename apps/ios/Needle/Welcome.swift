import SwiftUI

/// The first start: what Needle is, and where the music can come from, in the Settings style:
/// each choice a row with an icon square, a title, and one short line; Look around first as
/// the one clear button; crash reports as a row with a switch.
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
                VStack(alignment: .leading, spacing: 22) {
                    VStack(alignment: .leading, spacing: 10) {
                        Group {
                            if let icon = UIImage(named: "AppIcon") {
                                Image(uiImage: icon).resizable()
                            } else {
                                // The app's icon is not a picture the app can load: Needle's note, as in Settings.
                                Image(systemName: "music.note").font(.system(size: 32, weight: .bold)).foregroundStyle(.white)
                                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                                    .background(LinearGradient(colors: [store.colors.accent, store.colors.accent.opacity(0.6)], startPoint: .topLeading, endPoint: .bottomTrailing))
                            }
                        }
                        .frame(width: 72, height: 72).clipShape(.rect(cornerRadius: 17))
                        .accessibilityHidden(true)
                        Text("Welcome to Needle").font(.largeTitle.bold())
                        Text("Your music, from this iPhone, your server, and your computer.")
                            .font(.title3)
                        Text("No account. Nothing leaves your phone unless you ask.")
                            .font(.subheadline).foregroundStyle(.secondary)
                    }
                    .padding(.top, 50)

                    VStack(alignment: .leading, spacing: 8) {
                        Text("Where is your music?").font(.footnote.weight(.semibold)).foregroundStyle(.secondary)
                            .textCase(.uppercase).padding(.leading, 16)
                        VStack(spacing: 0) {
                            choice("folder.fill", .orange, "Import from Files", "Songs on this iPhone or in iCloud Drive.") { importing = true }
                            Divider().padding(.leading, 58)
                            choice("server.rack", .cyan, "Connect a music server", "Navidrome or another Subsonic server.") { finish(.home, .server("subsonic")) }
                            Divider().padding(.leading, 58)
                            choice("laptopcomputer", .indigo, "Connect your computer", "Play its music here, and move what plays.") { finish(.home, .computer) }
                        }
                        .glassEffect(.regular, in: .rect(cornerRadius: 22))
                    }

                    Button { finish(.home, nil) } label: {
                        Text("Look Around First").font(.headline).frame(maxWidth: .infinity).padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)

                    Toggle(isOn: $crashReports) {
                        SettingLabel(title: "Send crash reports", detail: "What went wrong. No music, names, or files.",
                                     icon: "exclamationmark.triangle.fill", tint: .orange)
                    }
                    .padding(.horizontal, 16).padding(.vertical, 12)
                    .glassEffect(.regular, in: .rect(cornerRadius: 22))
                    .onChange(of: crashReports) { _, on in store.run { try $0.setCrashReports(on: on) } }
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 30)
            }
        }
        .preferredColorScheme(.dark)
        .fileImporter(isPresented: $importing, allowedContentTypes: [.audio, .folder], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { store.importFiles(urls) }
            finish(.home, nil)
        }
    }

    /// A choice as a row: its icon square, its title, one short line, and an arrow.
    private func choice(_ symbol: String, _ tint: Color, _ title: String, _ detail: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 14) {
                Image(systemName: symbol).font(.system(size: 15, weight: .semibold)).foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(tint.gradient, in: .rect(cornerRadius: 8))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.body.weight(.semibold))
                    Text(detail).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 8)
                Image(systemName: "chevron.right").font(.footnote.weight(.bold)).foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 14).padding(.vertical, 13)
            .contentShape(.rect)
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
