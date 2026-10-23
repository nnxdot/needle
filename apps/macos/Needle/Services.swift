import SwiftUI

/// Settings › Online services: updates, Last.fm, ListenBrainz, and AcoustID.
struct ServicesSettings: View {
    private var store: Store { .shared }
    @State private var info: ServiceInfo?
    @State private var busy = false
    @State private var lastfmKey = ""
    @State private var lastfmSecret = ""
    @State private var token = ""
    @State private var acoustid = ""
    @State private var pending: String?
    /// Sign-in forms, shown only when asked for (Connect), until Cancel or a sign-in.
    @State private var connecting: Set<String> = []

    var body: some View {
        let s = store.settings
        Section {
            SettingRow(title: "Check for updates automatically", detail: "Once a day. You have Needle \(store.version).", icon: "arrow.down.circle") {
                SettingSwitch(key: "check_updates", on: s.checkUpdates)
            }
            HStack {
                Button("Check now") { store.checkForUpdate(force: true) }
                if let update = store.update {
                    if update.installable {
                        Button("Update to \(update.version)") { store.installUpdate() }.buttonStyle(.borderedProminent)
                    } else {
                        Button("Get Needle \(update.version)") { openWebPage(update.page) }
                    }
                    Button("What Needle \(update.version) brings") { openWebPage(update.page) }.buttonStyle(.link)
                }
            }
        } header: {
            GroupHeader(title: "Updates")
        }
        Section {
            if let error = info?.status.storeError {
                Text("The Keychain could not be read: \(error)").foregroundStyle(store.plainColors.danger).padding(.vertical, 6)
            }
            lastfm(s)
            SettingRow(title: "Fix artist names for scrobbles", detail: "Apple Music's spelling of an artist. Your files stay.", icon: "pencil") {
                SettingSwitch(key: "scrobble_corrections", on: s.scrobbleCorrections)
            }
            listenbrainz(s)
        } header: {
            GroupHeader(title: "Listening services")
        }
        .task { refresh() }
        Section {
            acoustidSection
        } header: {
            GroupHeader(title: "Identifying songs")
        } footer: {
            Text("Keys and sessions are kept in the Keychain. Nothing is sent until you connect and turn on a service.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

    private func refresh() {
        Task {
            info = JSON.decode(ServiceInfo.self, await store.load { $0.servicesJson() })
            pending = store.core.lastfmPending()
        }
    }

    private func line(_ state: ServiceInfo.State?) -> some View {
        let text: String = {
            guard let state else { return "…" }
            if let reason = state.rejected { return "Needs signing in again · \(reason)" }
            if let user = state.user { return state.source == "Environment" ? "\(user) · set by an environment variable" : "Signed in as \(user)" }
            if state.source == "Environment" { return "Set by an environment variable" }
            if state.configured { return "Key saved" }
            return "Not connected"
        }()
        let color: Color = state?.rejected != nil ? store.plainColors.danger : state?.configured == true ? store.plainColors.success : store.plainColors.line
        return HStack(spacing: 6) {
            Circle().fill(color).frame(width: 7, height: 7)
            Text(text).font(.callout).foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func queue(_ q: ServiceInfo.Queue?, service: String) -> some View {
        if let q {
            if q.pending == 0 && q.failed == 0 && q.signInRequired == nil {
                if q.sent > 0 { Text("\(q.sent) listens sent").font(.caption).foregroundStyle(.secondary) }
            } else {
                HStack {
                    Text([q.pending > 0 ? "\(q.pending) waiting to send" : nil, q.failed > 0 ? "\(q.failed) failed" : nil].compactMap { $0 }.joined(separator: " · "))
                        .font(.caption)
                    if let error = q.signInRequired ?? q.lastError {
                        Text(error).font(.caption).foregroundStyle(store.plainColors.danger).lineLimit(1)
                    }
                    if q.failed > 0 {
                        Button("Retry failed") {
                            store.job { "\(try $0.retryScrobbles(service: service)) listens will be retried." }
                            refresh()
                        }
                        .controlSize(.small)
                    }
                }
            }
        }
    }

    private func lastfm(_ s: NeedleSettings) -> some View {
        let state = info?.status.lastfm
        let configured = state?.configured == true
        let open = connecting.contains("lastfm")
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                RowIcon(name: "music.note")
                VStack(alignment: .leading, spacing: 3) {
                    Text("Last.fm")
                    line(state)
                }
                Spacer()
                if configured && state?.source != "Environment" {
                    Button("Sign out") { signOut("lastfm") }
                } else if !configured && pending == nil {
                    if info?.status.lastfmApp.configured == true {
                        // Needle's own key (or one saved): nothing to fill in, so Connect opens
                        // Last.fm's sign-in page at once.
                        Button(busy ? "Opening…" : "Connect") { lastfmBegin() }.buttonStyle(.borderedProminent).disabled(busy)
                    } else {
                        connectButton("lastfm", open: open)
                    }
                }
                RowToggle(label: "Scrobble to Last.fm", isOn: Binding(get: { s.lastfmEnabled }, set: { store.set(["lastfm_enabled": $0]) }))
                    .toggleStyle(.switch).labelsHidden().disabled(!configured)
                    .help("Scrobble qualified listens to Last.fm")
            }
            if !configured && (open || pending != nil) {
                if let pending {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Approve Needle in the Last.fm page that just opened, then press Continue.").font(.callout)
                        HStack {
                            Button(busy ? "Checking…" : "Continue") { lastfmContinue() }.buttonStyle(.borderedProminent).disabled(busy)
                            Button("Open the page again") { openWebPage(pending) }
                            Button("Cancel") { store.core.lastfmCancel(); self.pending = nil; connecting.remove("lastfm") }
                        }
                    }
                    .padding(10)
                    .background(store.plainColors.raised, in: RoundedRectangle(cornerRadius: 8))
                } else {
                    if info?.status.lastfmApp.configured != true {
                        Text("Needle needs a Last.fm API account to sign in. Create one at last.fm/api/account/create, then paste its key and shared secret.")
                            .font(.callout).foregroundStyle(.secondary)
                        HStack {
                            TextField("API key", text: $lastfmKey, prompt: Text("Last.fm API key")).labelsHidden()
                            SecureField("Shared secret", text: $lastfmSecret, prompt: Text("Shared secret")).labelsHidden()
                        }
                        .textFieldStyle(.roundedBorder)
                    }
                    Button { lastfmBegin() } label: { Label(busy ? "Opening…" : "Sign in with Last.fm", systemImage: "arrow.up.forward.app") }
                        .disabled(busy)
                }
            }
            queue(info?.summary?.lastfm, service: "lastfm")
        }
        .padding(.vertical, 4)
    }

    private func listenbrainz(_ s: NeedleSettings) -> some View {
        let state = info?.status.listenbrainz
        let configured = state?.configured == true
        let open = connecting.contains("listenbrainz")
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                RowIcon(name: "brain")
                VStack(alignment: .leading, spacing: 3) {
                    Text("ListenBrainz")
                    line(state)
                }
                Spacer()
                if configured && state?.source != "Environment" {
                    Button("Sign out") { signOut("listenbrainz") }
                } else if !configured {
                    connectButton("listenbrainz", open: open)
                }
                RowToggle(label: "Submit to ListenBrainz", isOn: Binding(get: { s.listenbrainzEnabled }, set: { store.set(["listenbrainz_enabled": $0]) }))
                    .toggleStyle(.switch).labelsHidden().disabled(!configured)
                    .help("Submit qualified listens to ListenBrainz")
            }
            if !configured && open {
                Text("Paste the user token from listenbrainz.org/settings.").font(.callout).foregroundStyle(.secondary)
                HStack {
                    SecureField("Token", text: $token, prompt: Text("ListenBrainz user token")).labelsHidden().textFieldStyle(.roundedBorder)
                    Button(busy ? "Checking…" : "Connect") {
                        let token = token.trimmingCharacters(in: .whitespaces)
                        guard !token.isEmpty else { store.fail("Paste your ListenBrainz user token first."); return }
                        busy = true
                        Task {
                            if let user = await store.ask({ try $0.listenbrainzSignIn(token: token) }) {
                                store.notify("Connected to ListenBrainz as \(user).")
                                connecting.remove("listenbrainz")
                            }
                            busy = false
                            refresh()
                            store.reloadSettings()
                        }
                    }
                    .disabled(busy)
                }
            }
            queue(info?.summary?.listenbrainz, service: "listenbrainz")
        }
        .padding(.vertical, 4)
    }

    private var acoustidSection: some View {
        let state = info?.status.acoustid
        let open = connecting.contains("acoustid")
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                RowIcon(name: "waveform.badge.magnifyingglass")
                VStack(alignment: .leading, spacing: 3) {
                    Text("AcoustID")
                    Text("Identifies songs by their sound.").font(.callout).foregroundStyle(.secondary)
                    line(state)
                }
                Spacer()
                if state?.configured == true && state?.source != "Environment" {
                    Button("Remove key") {
                        store.job { try $0.clearAcoustidKey(); return "AcoustID key removed." }
                        refresh()
                    }
                } else if state?.configured != true {
                    connectButton("acoustid", open: open)
                }
            }
            if state?.configured != true && open {
                Text("A free application key from acoustid.org.").font(.callout).foregroundStyle(.secondary)
                HStack {
                    SecureField("Key", text: $acoustid, prompt: Text("AcoustID application key")).labelsHidden().textFieldStyle(.roundedBorder)
                    Button("Save key") {
                        let key = acoustid.trimmingCharacters(in: .whitespaces)
                        guard !key.isEmpty else { store.fail("Paste your AcoustID application key first."); return }
                        store.job { try $0.setAcoustidKey(key: key); return "AcoustID key saved." }
                        connecting.remove("acoustid")
                        Task { try? await Task.sleep(for: .milliseconds(300)); refresh() }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    /// Connect opens a service's form; Cancel closes it again.
    @ViewBuilder
    private func connectButton(_ service: String, open: Bool) -> some View {
        if open {
            Button("Cancel") { connecting.remove(service) }
        } else {
            Button("Connect") { connecting.insert(service) }.buttonStyle(.borderedProminent)
        }
    }

    private func signOut(_ service: String) {
        Task {
            if await store.ask({ try $0.signOut(service: service) }) != nil {
                store.notify(service == "lastfm" ? "Signed out of Last.fm." : "Signed out of ListenBrainz.")
            }
            refresh()
            store.reloadSettings()
        }
    }

    private func lastfmBegin() {
        busy = true
        let (key, secret) = (lastfmKey, lastfmSecret)
        Task {
            if let url = await store.ask({ try $0.lastfmSignIn(key: key, secret: secret) }) {
                pending = url
                openWebPage(url)
            }
            busy = false
        }
    }

    private func lastfmContinue() {
        busy = true
        Task {
            if let user = await store.ask({ try $0.lastfmContinue() }) {
                pending = nil
                connecting.remove("lastfm")
                store.notify("Signed in to Last.fm as \(user). Scrobbling is on.")
                store.reloadSettings()
            }
            busy = false
            refresh()
        }
    }
}

/// Settings › Library › Music servers: Needle's own (Navidrome and other Subsonic servers),
/// then any a plugin adds. No plugin to add or turn on first.
struct MusicServersSection: View {
    private var store: Store { .shared }

    var body: some View {
        let servers = store.plugins.filter { $0.source != nil && ($0.builtIn || $0.enabled) }
            .sorted { $0.builtIn && !$1.builtIn }
        Section {
            if servers.isEmpty {
                Text("Needle is getting ready to sign in to music servers.").foregroundStyle(.secondary)
            }
            ForEach(servers, id: \.id) { plugin in SourceCard(plugin: plugin) }
        } header: {
            GroupHeader(title: "Music servers")
        }
        .task { store.reloadPlugins() }
    }
}

/// Settings › Plugins: every plugin with what it may do. Parts built into Needle (its music
/// servers) are not plugins here.
struct PluginsSettings: View {
    private var store: Store { .shared }
    @State private var updates: [PluginUpdate] = []

    var body: some View {
        let plugins = store.plugins.filter { !$0.builtIn }
        Section {
            if plugins.isEmpty {
                Text("No plugins yet. Add the examples, or put a plugin folder in the plugins folder and press Reload.")
                    .foregroundStyle(.secondary)
            }
            ForEach(plugins, id: \.id) { plugin in
                PluginCard(plugin: plugin, update: updates.first { $0.id == plugin.id }?.update) {
                    updates = JSON.decode([PluginUpdate].self, store.core.pluginUpdatesJson(), plain: true) ?? []
                }
            }
            HStack {
                Button("Add example plugins") {
                    Task {
                        if let n = await store.ask({ try $0.installBundledPlugins() }) {
                            store.notify(n == 0 ? "The example plugins are already installed." : "Added \(n) example plugins. Turn one on to try it.")
                            try? await Task.sleep(for: .milliseconds(400))
                            store.reloadPlugins()
                        }
                    }
                }
                Button("Reload") {
                    store.core.reloadPlugins()
                    store.notify("Plugins reloaded.")
                    Task { try? await Task.sleep(for: .milliseconds(400)); store.reloadPlugins() }
                }
                Button("Open plugins folder") {
                    let folder = store.core.pluginsFolder()
                    try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
                    NSWorkspace.shared.open(URL(fileURLWithPath: folder))
                }
            }
        } header: {
            GroupHeader(title: "Plugins", detail: "Add features with small scripts. Each plugin lists what it may do. Music servers are in Settings › Library.")
        } footer: {
            Text("Plugins are Rhai scripts in their own folders. Nothing runs until you turn it on. Learn to write one at needle.nnx.fyi/plugins.")
                .font(.caption).foregroundStyle(.secondary)
        }
        .task {
            store.reloadPlugins()
            updates = JSON.decode([PluginUpdate].self, store.core.pluginUpdatesJson(), plain: true) ?? []
        }
    }
}

private struct PluginCard: View {
    private var store: Store { .shared }
    let plugin: PluginItem
    let update: String?
    let changed: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(plugin.name).font(.body.weight(.semibold))
                        Text(plugin.version).font(.caption).foregroundStyle(.secondary)
                        Text(plugin.official ? "Official" : "Community")
                            .font(.caption2.weight(.medium))
                            .padding(.horizontal, 6).padding(.vertical, 1)
                            .background(plugin.official ? store.plainColors.accentSoft : store.plainColors.raised, in: Capsule())
                        if plugin.verified {
                            Image(systemName: "checkmark.seal.fill").foregroundStyle(store.plainColors.accent).help("Verified: comes with Needle and is unchanged")
                        }
                    }
                    if !plugin.author.isEmpty { Text("By \(plugin.author)").font(.caption).foregroundStyle(.secondary) }
                    Text(plugin.description).font(.callout).foregroundStyle(.secondary)
                }
                Spacer()
                RowToggle(label: plugin.name, isOn: Binding(get: { plugin.enabled }, set: { on in
                    store.core.setPluginEnabled(id: plugin.id, on: on)
                    Task { try? await Task.sleep(for: .milliseconds(400)); store.reloadPlugins() }
                }))
                .toggleStyle(.switch).labelsHidden()
                .help("Turning it on allows everything listed below.")
            }
            if let error = plugin.error {
                Text(error).font(.caption).foregroundStyle(store.plainColors.danger)
            }
            Text(plugin.permissions.isEmpty ? "Needs no permissions." : plugin.permissions.joined(separator: " · "))
                .font(.caption).foregroundStyle(.secondary)
            if plugin.findsLyrics { Text("Finds lyrics").font(.caption).foregroundStyle(.secondary) }
            if !plugin.effects.isEmpty { Text("Adds to Sound › Effects: \(plugin.effects.joined(separator: ", ")).").font(.caption).foregroundStyle(.secondary) }
            if let update {
                HStack {
                    Text("Version \(update) is out. You changed this plugin, so Needle left it alone. Updating keeps your changed files next to it as .mine files.")
                        .font(.caption)
                    Button("Update") { store.core.pluginUpdate(id: plugin.id, take: true); refresh() }
                    Button("Keep mine") { store.core.pluginUpdate(id: plugin.id, take: false); refresh() }
                }
                .controlSize(.small)
            }
        }
        .padding(.vertical, 4)
    }

    private func refresh() {
        Task { try? await Task.sleep(for: .milliseconds(400)); store.reloadPlugins(); changed() }
    }
}

/// A music server: sign in, its switches, its songs, and the songs kept on this Mac.
private struct SourceCard: View {
    private var store: Store { .shared }
    let plugin: PluginItem
    @State private var fields: [String: String] = [:]
    @State private var usage: [UInt64] = [0, 0]
    /// Songs played lately, kept for a while to play again without the network: count, bytes.
    @State private var cached: [UInt64] = [0, 0]
    /// The sign-in form, shown only after Sign in… (test builds: `-open-signin YES` opens it).
    @State private var signingIn: Bool = {
        #if DEBUG
        UserDefaults.standard.bool(forKey: "open-signin")
        #else
        false
        #endif
    }()
    /// The server's albums and artists, counted from its songs in the library.
    @State private var counts: (albums: Int, artists: Int)?
    @State private var confirmSignOut = false

    var body: some View {
        let source = plugin.source!
        Group {
            if source.signedIn {
                signedIn(source)
            } else {
                signedOut(source)
            }
        }
        .padding(.vertical, 4)
        .task { loadUsage() }
        .task(id: "\(source.songs)|\(source.syncing)") {
            guard source.signedIn else { return }
            // The core counts albums and artists with the server's rule, as the iPhone shows them.
            let id = plugin.id
            if let found = await store.load({ try $0.sourceStats(plugin: id) }), found.count == 2 {
                counts = (Int(found[0]), Int(found[1]))
            }
        }
        .confirmationDialog("Sign out of \(source.name)?", isPresented: $confirmSignOut) {
            Button("Sign Out", role: .destructive) { store.core.sourceSignOut(plugin: plugin.id); refresh() }
        } message: {
            Text("Its songs stay in your library, marked missing, until you sign in again.")
        }
    }

    // MARK: Signed in

    private func signedIn(_ source: MusicSource) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 14) {
                ServerBadge()
                VStack(alignment: .leading, spacing: 3) {
                    Text(source.name).font(.title3.weight(.semibold))
                    status(source)
                }
                Spacer()
                Button("Show songs") { store.navigate(.source(plugin: plugin.id, name: source.name)) }
                    .buttonStyle(.borderedProminent)
                Button(source.syncing ? "Updating…" : "Update now") { store.core.sourceSync(plugin: plugin.id) }
                    .disabled(source.syncing)
                    .help("Ask the server for its songs again")
                Button("Refresh everything…") {
                    if confirm("Refresh everything?", "Needle gets every song and cover again. Songs no longer on the server leave your library.", "Refresh") {
                        store.core.sourceRefresh(plugin: plugin.id)
                    }
                }
                .disabled(source.syncing)
                .help("Every song and cover again, moving covers too. Songs no longer on the server leave the library; Update now keeps them, marked missing.")
            }
            HStack(spacing: 10) {
                Stat(value: source.songs.formatted(), label: "Songs", symbol: "music.note")
                Stat(value: counts.map { $0.albums.formatted() } ?? "…", label: "Albums", symbol: "square.stack")
                Stat(value: counts.map { $0.artists.formatted() } ?? "…", label: "Artists", symbol: "music.microphone")
                Stat(value: source.syncedAt.map { Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named)) } ?? "Not yet",
                     label: "Updated", symbol: "clock")
            }
            SettingRow(title: "Kept offline", detail: "Songs you chose to keep. They play without the network.", icon: "arrow.down.circle") {
                HStack(spacing: 12) {
                    Text(sizeText(usage)).foregroundStyle(.secondary).monospacedDigit()
                    if (usage.first ?? 0) > 0 { Button("Stop Keeping All…") { stopKeeping() } }
                }
            }
            SettingRow(title: "Played lately", detail: "They play again without the network, for a while.", icon: "clock.arrow.circlepath") {
                HStack(spacing: 12) {
                    Text(sizeText(cached)).foregroundStyle(.secondary).monospacedDigit()
                    if (cached.first ?? 0) > 0 { Button("Free Space…") { freeSpace() } }
                }
            }
            if source.coversLeft > 0 {
                SettingRow(title: "Bringing in covers", detail: "\(source.coversLeft.formatted()) to go.", icon: "photo.on.rectangle") { ProgressView().controlSize(.small) }
            }
            ForEach(source.switches, id: \.id) { s in
                SettingRow(title: s.label, detail: s.detail, icon: "magnifyingglass") {
                    RowToggle(isOn: Binding(get: { s.on }, set: { on in store.core.sourceSwitch(plugin: plugin.id, id: s.id, on: on); refresh() }))
                        .toggleStyle(.switch).labelsHidden()
                }
            }
            if let error = source.error {
                Text(error).font(.caption).foregroundStyle(store.plainColors.danger)
            }
            HStack {
                Spacer()
                Button("Sign Out…", role: .destructive) { confirmSignOut = true }
                    .help("Forget the password. Its songs stay, marked missing, until you sign in again.")
            }
        }
    }

    /// How the server is doing: connected, updating its song list, or not reached.
    @ViewBuilder
    private func status(_ source: MusicSource) -> some View {
        if source.syncing {
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text(source.received > 0 ? "Updating the song list… \(source.received.formatted()) so far" : "Asking the server for its songs…")
                    .font(.callout).foregroundStyle(.secondary)
            }
        } else if let error = source.error {
            HStack(spacing: 6) {
                Circle().fill(store.plainColors.danger).frame(width: 7, height: 7)
                Text("Can't reach the server").font(.callout).foregroundStyle(store.plainColors.danger).help(error)
            }
        } else {
            HStack(spacing: 6) {
                Circle().fill(store.plainColors.success).frame(width: 7, height: 7)
                Text("Connected").font(.callout).foregroundStyle(.secondary)
            }
        }
    }

    // MARK: Signed out

    private func signedOut(_ source: MusicSource) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                RowIcon(name: "server.rack")
                VStack(alignment: .leading, spacing: 2) {
                    Text(source.name)
                    Text("Not signed in. Your server's songs show with your own.").font(.callout).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer()
                if !signingIn {
                    Button("Sign in…") { signingIn = true }.buttonStyle(.borderedProminent)
                }
            }
            if signingIn {
                signInForm(source)
            } else if let error = source.error {
                Text(error).font(.caption).foregroundStyle(store.plainColors.danger)
            }
        }
    }

    /// The sign-in form, as a Mac account sheet: one box, the labels right-aligned in a column
    /// beside the fields, the notes small and grey, and Cancel and Sign In at the bottom right
    /// (Return signs in).
    private func signInForm(_ source: MusicSource) -> some View {
        let p = store.plainColors
        return VStack(alignment: .leading, spacing: 12) {
            Text("Navidrome, Airsonic, Gonic, and other Subsonic servers.").font(.caption).foregroundStyle(.secondary)
            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 8) {
                ForEach(source.fields, id: \.id) { field in
                    GridRow {
                        Text("\(field.label):")
                            .foregroundStyle(.secondary)
                            .gridColumnAlignment(.trailing)
                        let binding = Binding(get: { fields[field.id] ?? "" }, set: { fields[field.id] = $0 })
                        let hint = Text(field.placeholder.isEmpty ? field.label : field.placeholder)
                        Group {
                            if field.secret {
                                SecureField(field.label, text: binding, prompt: hint)
                            } else {
                                TextField(field.label, text: binding, prompt: hint)
                            }
                        }
                        .labelsHidden()
                        .textFieldStyle(.roundedBorder)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: 340)
                        .onSubmit { signIn(source) }
                    }
                }
            }
            if let error = source.error {
                Text(error).font(.caption).foregroundStyle(p.danger)
            }
            HStack(alignment: .center) {
                Text("Passwords are kept in the Keychain, not in Needle's files.").font(.caption).foregroundStyle(.secondary)
                Spacer(minLength: 12)
                Button("Cancel") { signingIn = false }
                    .keyboardShortcut(.cancelAction)
                Button("Sign In") { signIn(source) }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(14)
        .background(p.raised.opacity(0.55), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(p.line.opacity(0.6), lineWidth: 1))
    }

    private func signIn(_ source: MusicSource) {
        store.core.sourceSignIn(plugin: plugin.id, fields: fields)
        store.notify("Signing in to \(source.name)…")
        refresh()
    }

    private func refresh() {
        Task {
            for _ in 0..<6 {
                try? await Task.sleep(for: .seconds(1))
                store.reloadPlugins()
            }
            store.libraryChanged()
        }
    }

    private func loadUsage() {
        usage = store.core.keptUsage(plugin: plugin.id)
        cached = store.core.streamCacheUsage(plugin: plugin.id)
    }

    private func sizeText(_ numbers: [UInt64]) -> String {
        "\((numbers.first ?? 0).formatted()) songs · \(ByteCountFormatter.string(fromByteCount: Int64(numbers.last ?? 0), countStyle: .file))"
    }

    /// Asks first, in the Mac's own alert; true for the first button.
    private func confirm(_ title: String, _ text: String, _ button: String) -> Bool {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = text
        alert.addButton(withTitle: button)
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.hasDestructiveAction = true
        alert.buttons.last?.keyEquivalent = "\u{1b}"
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func stopKeeping() {
        guard confirm("Stop keeping \((usage.first ?? 0).formatted()) songs?",
                      "They stay on this Mac until the space is needed, then they stream again.", "Stop Keeping All") else { return }
        let id = plugin.id
        Task {
            if let n = await store.ask({ try $0.forgetKept(plugin: id) }) { store.notify("\(n.formatted()) songs no longer kept.") }
            loadUsage()
        }
    }

    private func freeSpace() {
        guard confirm("Free \(ByteCountFormatter.string(fromByteCount: Int64(cached.last ?? 0), countStyle: .file))?",
                      "These songs stream again the next time you play them. Songs you kept stay.", "Free Space") else { return }
        let id = plugin.id
        Task {
            if let n = await store.ask({ try $0.clearStreamCache(plugin: id) }) { store.notify("Freed \(n.formatted()) songs.") }
            loadUsage()
        }
    }
}

/// The server's badge: a server in a rounded square of its own colors.
private struct ServerBadge: View {
    var body: some View {
        Image(systemName: "server.rack")
            .font(.system(size: 20, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: 44, height: 44)
            .background(LinearGradient(colors: [.cyan, .blue], startPoint: .topLeading, endPoint: .bottomTrailing), in: RoundedRectangle(cornerRadius: 11, style: .continuous))
            .accessibilityHidden(true)
    }
}

/// A number about the server: the number, and what it counts.
private struct Stat: View {
    private var store: Store { .shared }
    let value: String
    let label: String
    let symbol: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Image(systemName: symbol).font(.caption).foregroundStyle(.secondary)
            Text(value).font(.title3.weight(.semibold)).monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
            Text(label).font(.caption).foregroundStyle(.secondary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(store.plainColors.raised, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .accessibilityElement(children: .combine)
    }
}

/// Opens a web page whose address came from the network (an update's page, a sign-in page):
/// only https pages, and an address that does not parse is left alone.
@MainActor
func openWebPage(_ address: String) {
    guard let url = URL(string: address), url.scheme?.lowercased() == "https", url.host != nil else {
        Store.shared.fail("That page's address is not a web page, so Needle did not open it.")
        return
    }
    NSWorkspace.shared.open(url)
}
