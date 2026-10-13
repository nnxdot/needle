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

    var body: some View {
        let s = store.settings
        Section {
            SettingRow(title: "Check for updates automatically", detail: "Once a day, when Needle starts. You have Needle \(store.version). Needle asks needle.nnx.fyi for a newer version; nothing else is sent.") {
                SettingSwitch(key: "check_updates", on: s.checkUpdates)
            }
            HStack {
                Button("Check now") { store.checkForUpdate(force: true) }
                if let update = store.update {
                    if update.installable {
                        Button("Update to \(update.version)") { store.installUpdate() }.buttonStyle(.borderedProminent)
                    } else {
                        Button("Get Needle \(update.version)") { NSWorkspace.shared.open(URL(string: update.page)!) }
                    }
                    Button("What Needle \(update.version) brings") { NSWorkspace.shared.open(URL(string: update.page)!) }.buttonStyle(.link)
                }
            }
        } header: {
            GroupHeader(title: "Updates")
        }
        Section {
            if let error = info?.status.storeError {
                Text("The Keychain could not be read: \(error)").foregroundStyle(store.colors.danger).padding(.vertical, 6)
            }
            lastfm(s)
            SettingRow(title: "Fix artist names for scrobbles", detail: "When your tags write an artist another way than Apple Music does (키키 for KiiiKiii), scrobble Apple's spelling, so Last.fm and ListenBrainz file the song under the right artist. Your files are not changed.") {
                SettingSwitch(key: "scrobble_corrections", on: s.scrobbleCorrections)
            }
            listenbrainz(s)
        } header: {
            GroupHeader(title: "Listening services", detail: "Optional. Nothing is sent until you connect a service and turn it on.")
        }
        .task { refresh() }
        Section {
            acoustidSection
        } header: {
            GroupHeader(title: "Identifying songs")
        } footer: {
            Text("Keys and sessions are stored in the Keychain, never in your library or its exports. Environment variables override them.")
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
        let color: Color = state?.rejected != nil ? store.colors.danger : state?.configured == true ? store.colors.success : store.colors.line
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
                        Text(error).font(.caption).foregroundStyle(store.colors.danger).lineLimit(1)
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
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 20) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Last.fm").font(.body.weight(.medium))
                    line(state)
                }
                Spacer()
                if configured && state?.source != "Environment" {
                    Button("Sign out") { signOut("lastfm") }
                }
                Toggle("", isOn: Binding(get: { s.lastfmEnabled }, set: { store.set(["lastfm_enabled": $0]) }))
                    .toggleStyle(.switch).labelsHidden().disabled(!configured)
                    .help("Scrobble qualified listens to Last.fm")
            }
            if !configured {
                if let pending {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Approve Needle in the Last.fm page that just opened, then press Continue.").font(.callout)
                        HStack {
                            Button(busy ? "Checking…" : "Continue") { lastfmContinue() }.buttonStyle(.borderedProminent).disabled(busy)
                            Button("Open the page again") { NSWorkspace.shared.open(URL(string: pending)!) }
                            Button("Cancel") { store.core.lastfmCancel(); self.pending = nil }
                        }
                    }
                    .padding(10)
                    .background(store.colors.raised, in: RoundedRectangle(cornerRadius: 8))
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
        return VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 20) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("ListenBrainz").font(.body.weight(.medium))
                    line(state)
                }
                Spacer()
                if configured && state?.source != "Environment" {
                    Button("Sign out") { signOut("listenbrainz") }
                }
                Toggle("", isOn: Binding(get: { s.listenbrainzEnabled }, set: { store.set(["listenbrainz_enabled": $0]) }))
                    .toggleStyle(.switch).labelsHidden().disabled(!configured)
                    .help("Submit qualified listens to ListenBrainz")
            }
            if !configured {
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
        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text("AcoustID").font(.body.weight(.medium))
                    Text("Identifies tracks by their sound. Needs a free application key from acoustid.org.").font(.callout).foregroundStyle(.secondary)
                    line(state)
                }
                Spacer()
                if state?.configured == true && state?.source != "Environment" {
                    Button("Remove key") {
                        store.job { try $0.clearAcoustidKey(); return "AcoustID key removed." }
                        refresh()
                    }
                }
            }
            if state?.configured != true {
                HStack {
                    SecureField("Key", text: $acoustid, prompt: Text("AcoustID application key")).labelsHidden().textFieldStyle(.roundedBorder)
                    Button("Save key") {
                        let key = acoustid.trimmingCharacters(in: .whitespaces)
                        guard !key.isEmpty else { store.fail("Paste your AcoustID application key first."); return }
                        store.job { try $0.setAcoustidKey(key: key); return "AcoustID key saved." }
                        Task { try? await Task.sleep(for: .milliseconds(300)); refresh() }
                    }
                }
            }
        }
        .padding(.vertical, 4)
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
                NSWorkspace.shared.open(URL(string: url)!)
            }
            busy = false
        }
    }

    private func lastfmContinue() {
        busy = true
        Task {
            if let user = await store.ask({ try $0.lastfmContinue() }) {
                pending = nil
                store.notify("Signed in to Last.fm as \(user). Scrobbling is on.")
                store.reloadSettings()
            }
            busy = false
            refresh()
        }
    }
}

/// Settings › Plugins: music servers to sign in to, and every plugin with what it may do.
struct PluginsSettings: View {
    private var store: Store { .shared }
    @State private var updates: [PluginUpdate] = []

    var body: some View {
        let sources = store.plugins.filter { $0.source != nil && $0.enabled }
        if !sources.isEmpty {
            Section {
                ForEach(sources, id: \.id) { plugin in SourceCard(plugin: plugin) }
            } header: {
                GroupHeader(title: "Music servers", detail: "Navidrome, Airsonic, Gonic, and other Subsonic servers. Your server's songs show with your own and play straight away.")
            }
        }
        Section {
            if store.plugins.isEmpty {
                Text("No plugins yet. Add the examples, or put a plugin folder in the plugins folder and press Reload.")
                    .foregroundStyle(.secondary)
            }
            ForEach(store.plugins, id: \.id) { plugin in
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
            GroupHeader(title: "Plugins", detail: sources.isEmpty
                ? "Add features with small scripts. To play music from your Navidrome, Airsonic, or Gonic server, add the example plugins and turn on Subsonic."
                : "Add features with small scripts. Each plugin lists what it may do.")
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
                            .background(plugin.official ? store.colors.accentSoft : store.colors.raised, in: Capsule())
                        if plugin.verified {
                            Image(systemName: "checkmark.seal.fill").foregroundStyle(store.colors.accent).help("Verified: comes with Needle and is unchanged")
                        }
                    }
                    if !plugin.author.isEmpty { Text("By \(plugin.author)").font(.caption).foregroundStyle(.secondary) }
                    Text(plugin.description).font(.callout).foregroundStyle(.secondary)
                }
                Spacer()
                Toggle("", isOn: Binding(get: { plugin.enabled }, set: { on in
                    store.core.setPluginEnabled(id: plugin.id, on: on)
                    Task { try? await Task.sleep(for: .milliseconds(400)); store.reloadPlugins() }
                }))
                .toggleStyle(.switch).labelsHidden()
                .help("Turning it on allows everything listed below.")
            }
            if let error = plugin.error {
                Text(error).font(.caption).foregroundStyle(store.colors.danger)
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

    var body: some View {
        let source = plugin.source!
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(source.name).font(.body.weight(.semibold))
                Spacer()
                if source.signedIn {
                    Button("Show songs") { store.navigate(.source(plugin: plugin.id, name: source.name)) }
                    Button(source.syncing ? "Getting the list of songs…" : "Update now") { store.core.sourceSync(plugin: plugin.id) }
                        .disabled(source.syncing)
                        .help("Get the server's list of songs again")
                    Button("Sign out") { store.core.sourceSignOut(plugin: plugin.id); refresh() }
                        .help("Forget the password. Its songs stay, marked missing, until you sign in again.")
                }
            }
            if source.signedIn {
                let when = source.syncedAt.map { " · updated \(Date(timeIntervalSince1970: TimeInterval($0)).formatted(.relative(presentation: .named)))" } ?? ""
                Text("Signed in. \(source.songs.formatted()) songs\(when).").font(.callout).foregroundStyle(.secondary)
                if source.syncing && source.received > 0 {
                    Text("\(source.received.formatted()) songs so far…").font(.caption).foregroundStyle(.secondary)
                }
                ForEach(source.switches, id: \.id) { s in
                    Toggle(isOn: Binding(get: { s.on }, set: { on in store.core.sourceSwitch(plugin: plugin.id, id: s.id, on: on); refresh() })) {
                        VStack(alignment: .leading) {
                            Text(s.label)
                            if !s.detail.isEmpty { Text(s.detail).font(.caption).foregroundStyle(.secondary) }
                        }
                    }
                    .toggleStyle(.switch)
                }
                Text("Songs stream as you play them. The last 2 GB you played stay on this Mac, so they play again without the network.")
                    .font(.caption).foregroundStyle(.secondary)
                if (usage.first ?? 0) > 0 {
                    Text("\(usage[0]) songs kept on this Mac · \(ByteCountFormatter.string(fromByteCount: Int64(usage.last ?? 0), countStyle: .file))")
                        .font(.caption)
                }
            } else {
                Text("Type your server's address and sign in below.").font(.callout).foregroundStyle(.secondary)
                ForEach(source.fields, id: \.id) { field in
                    if field.secret {
                        SecureField(field.label, text: Binding(get: { fields[field.id] ?? "" }, set: { fields[field.id] = $0 }), prompt: Text(field.placeholder))
                    } else {
                        TextField(field.label, text: Binding(get: { fields[field.id] ?? "" }, set: { fields[field.id] = $0 }), prompt: Text(field.placeholder))
                    }
                }
                .textFieldStyle(.roundedBorder)
                Button("Sign in") {
                    store.core.sourceSignIn(plugin: plugin.id, fields: fields)
                    store.notify("Signing in to \(source.name)…")
                    refresh()
                }
                .buttonStyle(.borderedProminent)
                Text("Passwords are kept in the Keychain, not in Needle's files.").font(.caption).foregroundStyle(.secondary)
            }
            if let error = source.error {
                Text(error).font(.caption).foregroundStyle(store.colors.danger)
            }
        }
        .padding(.vertical, 4)
        .task { usage = store.core.keptUsage(plugin: plugin.id) }
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
}
