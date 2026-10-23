import SwiftUI
import UniformTypeIdentifiers

/// Where the music comes from: this iPhone, music servers (Navidrome and other Subsonic
/// servers, built into Needle, then any a plugin adds), and Needle on a computer.
struct SourcesView: View {
    @Environment(Store.self) private var store
    @State private var songs: UInt32 = 0
    @State private var servers: [PluginItem] = []
    @State private var importing = false
    @State private var pcConnected = false
    @State private var pc: PcState?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                phone
                Text("Music servers").font(.title3.bold()).padding(.top, 14).padding(.horizontal, 4)
                if servers.isEmpty {
                    Card {
                        Text("Needle is getting ready to sign in to music servers.").foregroundStyle(.secondary)
                    }
                }
                ForEach(servers, id: \.id) { plugin in
                    if let source = plugin.source {
                        NavigationLink {
                            ServerView(plugin: plugin.id)
                        } label: {
                            SourceCard(symbol: "server.rack", tint: .cyan, title: plugin.name, detail: serverDetail(source), status: serverStatus(source))
                        }
                        .buttonStyle(Pressable())
                    }
                }
                Text("Your computer").font(.title3.bold()).padding(.top, 14).padding(.horizontal, 4)
                NavigationLink {
                    ComputerView()
                } label: {
                    SourceCard(symbol: "laptopcomputer", tint: .orange, title: "Needle on your computer",
                               detail: pcDetail, status: pcConnected ? ("Connected", .green) : nil)
                }
                .buttonStyle(Pressable())
            }
            .padding(.horizontal)
            .padding(.bottom, 24)
        }
        .navigationTitle("Sources")
        .themed()
        .fileImporter(isPresented: $importing, allowedContentTypes: [.audio, .folder], allowsMultipleSelection: true) { result in
            if case .success(let urls) = result { store.importFiles(urls) }
        }
        .task(id: store.libraryVersion) {
            while !Task.isCancelled {
                await reload()
                try? await Task.sleep(for: .seconds(1.5))
            }
        }
    }

    private var phone: some View {
        Card {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 14) {
                    Icon(symbol: "iphone", tint: .white)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("This iPhone").font(.headline)
                        Text(count(Int(songs), "song")).font(.subheadline).foregroundStyle(.secondary)
                        if store.scan.running {
                            Text("Reading… \(count(Int(store.scan.imported), "song")) so far").font(.caption).foregroundStyle(Color.accentColor)
                        } else if let error = store.scan.error {
                            Text(error).font(.caption).foregroundStyle(.red)
                        } else {
                            Text("● Ready").font(.caption).foregroundStyle(.green)
                        }
                    }
                }
                GlassEffectContainer {
                    HStack(spacing: 10) {
                        Button { importing = true } label: {
                            Label("Import from Files", systemImage: "folder.badge.plus").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.glassProminent)
                        Button { store.rescan() } label: {
                            Label("Read again", systemImage: "arrow.clockwise").labelStyle(.iconOnly).frame(width: 24)
                        }
                        .buttonStyle(.glass)
                        .disabled(store.scan.running)
                    }
                }
                Text("You can also put music in Files › On My iPhone › Needle, or copy it from a computer in Finder.")
                    .font(.footnote).foregroundStyle(.secondary)
            }
        }
    }

    private var pcDetail: String {
        guard pcConnected else { return "Play your computer's music here, and move what plays" }
        if let song = pc?.current { return (pc?.playing == true ? "Playing " : "Paused: ") + "“\(song.title)”" }
        return "Nothing playing"
    }

    private func serverDetail(_ source: MusicSource) -> String {
        source.signedIn ? count(Int(source.songs), "song") : "Not signed in"
    }

    private func serverStatus(_ source: MusicSource) -> (String, Color)? {
        if let error = source.error { return (error, .red) }
        if source.syncing { return ("Bringing in songs… \(source.received)", .accentColor) }
        guard source.signedIn else { return nil }
        if let at = source.syncedAt {
            let when = Date(timeIntervalSince1970: TimeInterval(at)).formatted(.relative(presentation: .named))
            return ("● Synced \(when)", .green)
        }
        return ("● Signed in", .green)
    }

    private func reload() async {
        songs = await store.load { try $0.songCount() } ?? 0
        // Needle's own server first, then any a plugin adds.
        servers = await store.load { $0.plugins().filter { $0.source != nil }.sorted { $0.builtIn && !$1.builtIn } } ?? []
        pcConnected = await store.load { $0.pcConnected() } ?? false
        // The Store already asks the computer: its answer, not another request.
        pc = pcConnected ? store.pc : nil
    }
}

/// A pane of glass around a source.
struct Card<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        content()
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .glassEffect(.regular, in: .rect(cornerRadius: 24))
    }
}

struct Icon: View {
    let symbol: String
    let tint: Color

    var body: some View {
        Image(systemName: symbol).font(.title3).foregroundStyle(tint)
            .frame(width: 46, height: 46)
            .glassEffect(.regular, in: .rect(cornerRadius: 14))
    }
}

struct SourceCard: View {
    let symbol: String
    let tint: Color
    let title: String
    let detail: String
    let status: (String, Color)?

    var body: some View {
        Card {
            HStack(spacing: 14) {
                Icon(symbol: symbol, tint: tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.headline).foregroundStyle(.primary)
                    Text(detail).font(.subheadline).foregroundStyle(.secondary).lineLimit(2)
                    if let status { Text(status.0).font(.caption).foregroundStyle(status.1).lineLimit(2) }
                }
                .multilineTextAlignment(.leading)
                Spacer()
                Image(systemName: "chevron.right").font(.footnote.weight(.bold)).foregroundStyle(.tertiary)
            }
        }
    }
}

/// A music server: a card with its name and how it is doing, its numbers, what is kept on this
/// iPhone, its actions and its own switches, and Sign out at the foot. Signed out: a sign-in
/// card.
struct ServerView: View {
    @Environment(Store.self) private var store
    let plugin: String
    @State private var item: PluginItem?
    @State private var fields: [String: String] = [:]
    @State private var kept: [UInt64] = []
    @State private var signingIn = false
    @State private var waiting: Task<Void, Never>?
    /// The server's songs, albums, and artists, counted from its songs in the library.
    @State private var counts: (songs: Int, albums: Int, artists: Int)?
    @State private var confirmSignOut = false
    /// Songs played lately, kept for a while to play again without the network: count, bytes.
    @State private var cached: [UInt64] = []
    @State private var confirmFree = false
    @State private var confirmStopKeeping = false

    /// Test builds: `-server-signed-out` shows the sign-in card even while signed in (nothing
    /// is signed out).
    private var showSignedOut: Bool {
        #if DEBUG
        UserDefaults.standard.bool(forKey: "server-signed-out")
        #else
        false
        #endif
    }

    /// Signs in, and waits for the server's answer: signed in, or an error (even the same one
    /// as before), or a minute at most.
    private func signIn() {
        guard let before = item?.source else { return }
        signingIn = true
        store.core.sourceSignIn(plugin: plugin, fields: fields)
        waiting?.cancel()
        waiting = Task {
            let sent = Date()
            while !Task.isCancelled, Date().timeIntervalSince(sent) < 60 {
                try? await Task.sleep(for: .milliseconds(500))
                let now = await store.load { core in core.plugins().first { $0.id == plugin }?.source } ?? nil
                guard let now else { break }
                // Signed in, a new error, or the same error after the server had time to answer.
                if now.signedIn || now.error != before.error || (now.error != nil && Date().timeIntervalSince(sent) > 3) { break }
            }
            if !Task.isCancelled { signingIn = false }
        }
    }

    var body: some View {
        ScrollViewReader { reader in
        Form {
            if let item, let source = item.source {
                if source.signedIn && !showSignedOut {
                    signedIn(item, source)
                } else {
                    signedOut(source)
                }
                // Needle's own server is part of Needle, not a plugin.
                if !item.builtIn {
                    Section("About") {
                        LabeledContent("Plugin", value: "\(item.name) \(item.version)")
                        if !item.author.isEmpty { LabeledContent("By", value: item.author) }
                        ForEach(item.permissions, id: \.self) { Text("• \($0)").font(.footnote) }
                    }
                }
            }
        }
        #if DEBUG
        // Test builds: -server-scroll space opens the page at what is kept on this iPhone.
        .task(id: item == nil) {
            guard item != nil, UserDefaults.standard.string(forKey: "server-scroll") == "space" else { return }
            try? await Task.sleep(for: .milliseconds(700))
            reader.scrollTo("space", anchor: .top)
        }
        #endif
        }
        .navigationTitle(item?.source?.name ?? item?.name ?? "Server")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
        .confirmationDialog("Sign out of \(item?.source?.name ?? "the server")?", isPresented: $confirmSignOut, titleVisibility: .visible) {
            Button("Sign Out", role: .destructive) { store.core.sourceSignOut(plugin: plugin) }
        } message: {
            Text("Its songs stay in your library, marked missing, until you sign in again.")
        }
        .task {
            while !Task.isCancelled {
                let id = plugin
                item = await store.load { $0.plugins().first { $0.id == id } } ?? nil
                kept = await store.load { $0.keptUsage(plugin: id) } ?? []
                cached = await store.load { $0.streamCacheUsage(plugin: id) } ?? []
                try? await Task.sleep(for: .seconds(1))
            }
        }
        .task(id: "\(item?.source?.songs ?? 0)|\(item?.source?.syncing ?? false)") {
            guard item?.source?.signedIn == true else { return }
            let found = await store.load { core -> [Int] in
                let songs = try core.songs().filter { $0.id.hasPrefix("src-") }
                // Albums by name: a song knows its own artist, not the album's, so counting name and
                // artist counted an album once for each guest artist.
                return [songs.count, Set(songs.map(\.album).filter { !$0.isEmpty }).count, Set(songs.map(\.artist)).count]
            }
            if let found, found.count == 3 { counts = (found[0], found[1], found[2]) }
        }
    }

    // MARK: Signed in

    @ViewBuilder
    private func signedIn(_ item: PluginItem, _ source: MusicSource) -> some View {
        Section {
            header(source.name, status: status(source))
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
        }
        Section {
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 10), GridItem(.flexible(), spacing: 10)], spacing: 10) {
                StatTile(value: Int(source.songs).formatted(), label: "Songs", symbol: "music.note")
                StatTile(value: counts.map { $0.albums.formatted() } ?? "…", label: "Albums", symbol: "square.stack")
                StatTile(value: counts.map { $0.artists.formatted() } ?? "…", label: "Artists", symbol: "music.microphone")
                StatTile(value: source.syncedAt.map { Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named)) } ?? "Not yet",
                         label: "Updated", symbol: "clock")
            }
            .listRowInsets(EdgeInsets())
            .listRowBackground(Color.clear)
        }
        Section {
            NavigationLink {
                SongList(title: source.name) { core in try core.songs().filter { $0.id.hasPrefix("src-") } }
            } label: {
                SettingLabel(title: "Show songs", detail: "Everything on your server, to play or keep.", icon: "music.note.list", tint: .cyan)
            }
            Button {
                store.core.sourceSync(plugin: plugin)
            } label: {
                HStack {
                    SettingLabel(title: source.syncing ? "Updating…" : "Update now", detail: "Ask the server for its songs again.", icon: "arrow.triangle.2.circlepath", tint: .blue)
                    Spacer()
                    if source.syncing { ProgressView() }
                }
            }
            .foregroundStyle(.primary)
            .disabled(source.syncing)
            if source.coversLeft > 0 {
                SettingLabel(title: "Bringing in covers", detail: "\(count(Int(source.coversLeft), "cover")) to go.", icon: "photo.on.rectangle", tint: .orange)
            }
        }
        Section {
            usage("Kept offline", "Songs you chose to keep.", "arrow.down.circle.fill", .green, kept)
            if (kept.first ?? 0) > 0 {
                Button("Stop Keeping All") { confirmStopKeeping = true }
                    .foregroundStyle(.tint)
                    .confirmationDialog("Stop keeping \(count(Int(kept.first ?? 0), "song"))?", isPresented: $confirmStopKeeping, titleVisibility: .visible) {
                        Button("Stop Keeping All", role: .destructive) {
                            let id = plugin
                            Task {
                                if let n = await store.ask({ try $0.forgetKept(plugin: id) }) {
                                    store.message = "\(count(Int(n), "song")) no longer kept"
                                }
                            }
                        }
                    } message: {
                        Text("They stay on this iPhone until the space is needed, then they stream again.")
                    }
            }
            usage("Played lately", "They play again without the network.", "clock.arrow.circlepath", .blue, cached)
            if (cached.first ?? 0) > 0 {
                Button("Free Space") { confirmFree = true }
                    .foregroundStyle(.tint)
                    .confirmationDialog("Free \(ByteCountFormatter.string(fromByteCount: Int64(cached.last ?? 0), countStyle: .file))?", isPresented: $confirmFree, titleVisibility: .visible) {
                        Button("Free Space", role: .destructive) {
                            let id = plugin
                            Task {
                                if let n = await store.ask({ try $0.clearStreamCache(plugin: id) }) {
                                    store.message = "Freed \(count(Int(n), "song"))"
                                }
                            }
                        }
                    } message: {
                        Text("These songs stream again the next time you play them. Songs you kept stay.")
                    }
            }
        } header: {
            Text("On this iPhone")
        } footer: {
            Text("Keep a song, album, or playlist from its menu: Keep Offline.")
        }
        .id("space")
        if !source.switches.isEmpty {
            Section("On the server") {
                ForEach(source.switches, id: \.id) { s in
                    Toggle(isOn: Binding(get: { s.on }, set: { store.core.sourceSwitch(plugin: plugin, id: s.id, on: $0) })) {
                        SettingLabel(title: s.label, detail: s.detail, icon: "magnifyingglass", tint: .purple)
                    }
                }
            }
        }
        Section {
            // Red, as a destructive button is (the page's text colour would cover it).
            Button("Sign Out", role: .destructive) { confirmSignOut = true }
                .foregroundStyle(.red)
                .frame(maxWidth: .infinity)
        }
    }

    // MARK: Signed out

    @ViewBuilder
    private func signedOut(_ source: MusicSource) -> some View {
        Section {
            VStack(alignment: .leading, spacing: 10) {
                header(source.name, status: nil)
                Text("Play the music on your own server, with your library: Navidrome, Airsonic, Gonic, and other Subsonic servers.")
                    .font(.subheadline).foregroundStyle(.secondary)
                    .padding(.horizontal, 16)
            }
            .listRowInsets(EdgeInsets())
            .listRowBackground(Color.clear)
        }
        Section {
            ForEach(source.fields, id: \.id) { field in
                let binding = Binding(get: { fields[field.id] ?? "" }, set: { fields[field.id] = $0 })
                // The field's name beside it, and its example (or its name) inside.
                let hint = Text(field.placeholder.isEmpty ? field.label : field.placeholder)
                HStack(spacing: 12) {
                    Text(field.label).frame(width: 126, alignment: .leading).lineLimit(1).minimumScaleFactor(0.8)
                    if field.secret {
                        SecureField(field.label, text: binding, prompt: hint)
                    } else {
                        TextField(field.label, text: binding, prompt: hint)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .keyboardType(field.id.contains("server") || field.id.contains("url") ? .URL : .default)
                    }
                }
            }
        } header: {
            Text("Your server")
        } footer: {
            Text("The whole address, with http:// or https://. Your password stays in the iPhone Keychain.")
        }
        Section {
            if signingIn {
                HStack(spacing: 10) { ProgressView(); Text("Signing in…") }
            } else {
                Button { signIn() } label: {
                    Text("Sign In").font(.headline).frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            }
            if !signingIn, let error = source.error { Text(error).foregroundStyle(.red) }
        }
    }

    // MARK: Pieces

    /// A row of songs on this iPhone: what they are, and how many and how much space.
    private func usage(_ title: String, _ detail: String, _ icon: String, _ tint: Color, _ numbers: [UInt64]) -> some View {
        HStack {
            SettingLabel(title: title, detail: detail, icon: icon, tint: tint)
            Spacer()
            VStack(alignment: .trailing, spacing: 1) {
                Text(count(Int(numbers.first ?? 0), "song")).monospacedDigit()
                Text(ByteCountFormatter.string(fromByteCount: Int64(numbers.last ?? 0), countStyle: .file)).font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private struct Status {
        let text: String
        let color: Color
        var busy = false
    }

    private func status(_ source: MusicSource) -> Status {
        if source.syncing {
            return Status(text: source.received > 0 ? "Updating the song list… \(count(Int(source.received), "song")) so far" : "Asking the server for its songs…", color: .blue, busy: true)
        }
        if let error = source.error { return Status(text: "Can't reach the server · \(error)", color: .red) }
        return Status(text: "Connected", color: .green)
    }

    /// The server's card: its icon and name in large type, and how it is doing.
    private func header(_ name: String, status: Status?) -> some View {
        HStack(spacing: 14) {
            Image(systemName: "server.rack")
                .font(.system(size: 26, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 58, height: 58)
                .background(LinearGradient(colors: [.cyan, .blue], startPoint: .topLeading, endPoint: .bottomTrailing), in: .rect(cornerRadius: 15))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(name).font(.title2.bold()).lineLimit(2)
                if let status {
                    HStack(spacing: 6) {
                        if status.busy {
                            ProgressView().controlSize(.mini)
                        } else {
                            Circle().fill(status.color).frame(width: 8, height: 8)
                        }
                        Text(status.text).font(.subheadline).foregroundStyle(status.color == .green ? .secondary : status.color).lineLimit(2)
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .glassEffect(.regular, in: .rect(cornerRadius: 22))
        .accessibilityElement(children: .combine)
    }
}

/// A number about the server, on glass: the number large, what it counts under it.
private struct StatTile: View {
    let value: String
    let label: String
    let symbol: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Image(systemName: symbol).font(.subheadline).foregroundStyle(.secondary)
            Text(value).font(.title3.bold()).monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
            Text(label).font(.caption).foregroundStyle(.secondary)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular, in: .rect(cornerRadius: 18))
        .accessibilityElement(children: .combine)
    }
}
