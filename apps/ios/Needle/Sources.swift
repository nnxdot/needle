import SwiftUI
import UniformTypeIdentifiers

/// Where the music comes from: this iPhone, music servers (Navidrome, Subsonic, through
/// Needle's plugins), and Needle on a computer.
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
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Navidrome and Subsonic servers come through Needle's plugins.").foregroundStyle(.secondary)
                            Button("Add Needle's plugins") {
                                Task {
                                    let added = await store.load { try $0.installBundledPlugins() } ?? 0
                                    store.message = added > 0 ? "Added \(count(Int(added), "plugin"))" : "Needle's plugins are all here"
                                    await reload()
                                }
                            }
                            .buttonStyle(.glass)
                        }
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
        servers = await store.load { $0.plugins().filter { $0.source != nil } } ?? []
        pcConnected = await store.load { $0.pcConnected() } ?? false
        pc = pcConnected ? await store.load { try $0.pcState() } : nil
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

/// A music server: sign in, bring its songs in again, and its own switches.
struct ServerView: View {
    @Environment(Store.self) private var store
    let plugin: String
    @State private var item: PluginItem?
    @State private var fields: [String: String] = [:]

    var body: some View {
        Form {
            if let item, let source = item.source {
                Section {
                    if source.signedIn {
                        LabeledContent("Songs", value: Int(source.songs).formatted())
                        if let at = source.syncedAt {
                            LabeledContent("Brought in", value: Date(timeIntervalSince1970: TimeInterval(at)).formatted(date: .abbreviated, time: .shortened))
                        }
                        if source.syncing {
                            HStack {
                                ProgressView()
                                Text(source.received > 0 ? "Bringing in the song list… \(count(Int(source.received), "song")) so far" : "Asking the server for its songs…")
                            }
                        } else if source.coversLeft > 0 {
                            Text("Bringing in covers… \(count(Int(source.coversLeft), "cover")) to go")
                        }
                        Button("Bring in songs again") { store.core.sourceSync(plugin: plugin) }.disabled(source.syncing)
                        Button("Sign out", role: .destructive) { store.core.sourceSignOut(plugin: plugin) }
                    } else {
                        ForEach(source.fields, id: \.id) { field in
                            let binding = Binding(get: { fields[field.id] ?? "" }, set: { fields[field.id] = $0 })
                            if field.secret {
                                SecureField(field.label, text: binding, prompt: Text(field.placeholder))
                            } else {
                                TextField(field.label, text: binding, prompt: Text(field.placeholder))
                                    .textInputAutocapitalization(.never)
                                    .autocorrectionDisabled()
                                    .keyboardType(.URL)
                            }
                        }
                        Button("Sign in") { store.core.sourceSignIn(plugin: plugin, fields: fields) }
                    }
                    if let error = source.error { Text(error).foregroundStyle(.red) }
                } header: {
                    Text(source.name)
                } footer: {
                    if !source.signedIn { Text("Type the server's whole address, with http:// or https://.") }
                }
                if !source.switches.isEmpty {
                    Section {
                        ForEach(source.switches, id: \.id) { s in
                            Toggle(isOn: Binding(get: { s.on }, set: { store.core.sourceSwitch(plugin: plugin, id: s.id, on: $0) })) {
                                Text(s.label)
                                if !s.detail.isEmpty { Text(s.detail) }
                            }
                        }
                    }
                }
                Section("About") {
                    LabeledContent("Plugin", value: "\(item.name) \(item.version)")
                    if !item.author.isEmpty { LabeledContent("By", value: item.author) }
                    ForEach(item.permissions, id: \.self) { Text("• \($0)").font(.footnote) }
                }
            }
        }
        .scrollContentBackground(.hidden)
        .navigationTitle(item?.name ?? "Server")
        .task {
            while !Task.isCancelled {
                let id = plugin
                item = await store.load { $0.plugins().first { $0.id == id } } ?? nil
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }
}

/// Needle on a computer: connect with the phone remote's address, see what it plays, play its
/// music here, and move what plays from one to the other.
struct ComputerView: View {
    @Environment(Store.self) private var store
    @State private var connected = false
    @State private var link = ""
    @State private var busy = false
    @State private var state: PcState?

    var body: some View {
        Form {
            if !connected {
                Section {
                    TextField("http://192.168.1.5:47380/r/…", text: $link)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                    Button("Paste") { link = UIPasteboard.general.string ?? link }
                    Button("Connect") { connect() }.disabled(busy || link.isEmpty)
                } footer: {
                    Text("On the computer, turn on the phone remote in Settings › Playback. Copy the address under its QR code, or scan the code with the Camera, then paste it here. Both need to be on the same network.")
                }
            } else {
                if let song = state?.current {
                    Section(state?.playing == true ? "Playing on the computer" : "Paused on the computer") {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(song.title).font(.headline)
                            Text(song.artist).foregroundStyle(.secondary)
                        }
                        HStack(spacing: 24) {
                            Spacer()
                            Button { pc("previous") } label: { Image(systemName: "backward.fill") }
                            Button { pc("toggle") } label: { Image(systemName: state?.playing == true ? "pause.fill" : "play.fill") }
                            Button { pc("next") } label: { Image(systemName: "forward.fill") }
                            Spacer()
                        }
                        .font(.title2)
                        .buttonStyle(.borderless)
                    }
                }
                Section {
                    Button("Play here", systemImage: "iphone") {
                        store.run { core in
                            let moved = try core.moveFromPc()
                            Task { @MainActor in store.message = moved ? "Playing here now" : "Nothing plays on the computer" }
                        }
                    }
                    Button("Move to the computer", systemImage: "laptopcomputer") {
                        store.run { core in
                            let missing = try core.moveToPc()
                            Task { @MainActor in
                                store.message = missing > 0 ? "\(count(Int(missing), "song")) are not on the computer" : "Playing on the computer now"
                            }
                        }
                    }
                }
                Section {
                    Button("Disconnect", role: .destructive) { store.core.disconnectPc(); connected = false }
                }
            }
        }
        .scrollContentBackground(.hidden)
        .navigationTitle("Your computer")
        .task {
            connected = await store.load { $0.pcConnected() || $0.reconnectPc() } ?? false
            while !Task.isCancelled {
                if connected { state = await store.load { try $0.pcState() } }
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    private func connect() {
        busy = true
        let address = link.trimmingCharacters(in: .whitespacesAndNewlines)
        Task {
            let core = store.core
            let result = await Task.detached { Result { try core.connectPc(link: address) } }.value
            busy = false
            switch result {
            case .success: connected = true; store.message = "Connected to Needle on your computer"
            case .failure(let error): store.message = error.localizedDescription
            }
        }
    }

    private func pc(_ action: String) {
        store.run { try $0.pcDo(action: action) }
    }
}

/// Needle's version and crash reports.
struct SettingsView: View {
    @Environment(Store.self) private var store
    @State private var crashReports = false

    var body: some View {
        Form {
            Section {
                Toggle("Send crash reports", isOn: Binding(get: { crashReports }, set: { on in
                    crashReports = on
                    store.run { try $0.setCrashReports(on: on) }
                }))
            } footer: {
                Text("When Needle stops by mistake, it sends what went wrong (no music, names, or files) the next time it opens.")
            }
            Section {
                Button("Read the music folder again") { store.rescan() }
            }
            Section {
                LabeledContent("Version", value: store.core.version())
            }
        }
        .scrollContentBackground(.hidden)
        .navigationTitle("Settings")
        .task {
            crashReports = await store.load { $0.crashReports() } ?? false
        }
    }
}
