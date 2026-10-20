import SwiftUI
import UniformTypeIdentifiers

/// Import: bring ratings, play counts, history, and playlists from other apps.
struct ImportPage: View {
    private var store: Store { .shared }
    @State private var found: [Detected] = []
    @State private var lastfmUser = ""
    @State private var listenbrainzUser = ""
    @State private var results: [String] = []

    var body: some View {
        let colors = store.colors
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Bring your music history").font(store.display(28))
                    Text("Ratings, play counts, listening history, and playlists come across. Your files are never changed, and importing again skips what is already here.")
                        .foregroundStyle(colors.ink2)
                    Text("Listens for songs you don't have are kept in your history, so your statistics stay complete. Imported listens are never sent to Last.fm or ListenBrainz.")
                        .font(.callout).foregroundStyle(colors.ink3)
                }
                if let progress = store.importProgress {
                    HStack {
                        ProgressView().controlSize(.small)
                        Text(progress)
                        Spacer()
                        Button("Stop") { store.core.cancelImport() }
                    }
                    .padding(12)
                    .background(colors.raised, in: RoundedRectangle(cornerRadius: 10))
                }
                if !found.isEmpty {
                    Text("Found on this Mac").font(.headline)
                    ForEach(found) { d in
                        source(d.label, d.detail, "sparkle.magnifyingglass") {
                            Button("Import") { run(kind(d.kind), d.path) }
                        }
                    }
                }
                Text("Choose a source").font(.headline).padding(.top, 6)
                source("Library XML", "Star ratings, play counts, last played and date added, and your playlists. Export first. Apple Music: File › Library › Export Library. MusicBee: Preferences › Library › export an iTunes XML file.", "music.note.house") {
                    Button("Choose library XML…") { pick("itunes", types: [.xml], folders: false) }
                }
                source("Spotify", "Every stream in your history, and your playlists, matched to the songs you own. Ask Spotify for your data at spotify.com/account/privacy.", "waveform") {
                    Button("Choose Spotify download…") { pick("spotify", types: [.zip], folders: true) }
                }
                source("Last.fm", "Every scrobble on your profile. Runs again later to fetch only new ones. Needs a Last.fm API key, set in Settings › Online services.", "dot.radiowaves.left.and.right") {
                    HStack {
                        TextField("Last.fm user name", text: $lastfmUser).textFieldStyle(.roundedBorder).frame(width: 180)
                        Button("Import") {
                            guard !lastfmUser.isEmpty else { store.fail("Enter a Last.fm user name first."); return }
                            run("lastfm", lastfmUser)
                        }
                    }
                }
                source("ListenBrainz", "Every public listen on your profile. No sign-in needed.", "ear") {
                    HStack {
                        TextField("ListenBrainz user name", text: $listenbrainzUser).textFieldStyle(.roundedBorder).frame(width: 180)
                        Button("Import") {
                            guard !listenbrainzUser.isEmpty else { store.fail("Enter a ListenBrainz user name first."); return }
                            run("listenbrainz", listenbrainzUser)
                        }
                    }
                }
                source("A folder of playlists", "Every .m3u and .m3u8 file in a folder, for example exported from foobar2000 or MusicBee.", "folder") {
                    Button("Choose folder…") { pick("folder", types: [], folders: true) }
                }
                if !results.isEmpty {
                    Text("Done").font(.headline).padding(.top, 6)
                    ForEach(results, id: \.self) { Label($0, systemImage: "checkmark.circle").foregroundStyle(colors.success) }
                }
            }
            .padding(28)
            .frame(maxWidth: 820, alignment: .leading)
        }
        .task { found = JSON.decode([Detected].self, await store.load { $0.importSourcesJson() }, plain: true) ?? [] }
    }

    private func kind(_ k: String) -> String {
        switch k {
        case "Itunes": "itunes"
        case "Spotify": "spotify"
        case "Lastfm": "lastfm"
        case "ListenBrainz": "listenbrainz"
        default: "folder"
        }
    }

    private func source<Action: View>(_ title: String, _ detail: String, _ icon: String, @ViewBuilder action: () -> Action) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: icon).font(.title2).foregroundStyle(store.colors.accent).frame(width: 30)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.body.weight(.semibold))
                Text(detail).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
            action()
        }
        .padding(14)
        .background(store.colors.raised.opacity(0.6), in: RoundedRectangle(cornerRadius: 12))
        .disabled(store.importProgress != nil)
    }

    private func pick(_ kind: String, types: [UTType], folders: Bool) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = !types.isEmpty
        panel.canChooseDirectories = folders
        if !types.isEmpty { panel.allowedContentTypes = types }
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        run(kind, path)
    }

    private func run(_ kind: String, _ path: String) {
        Task {
            if let summary = await store.ask({ try $0.importFrom(kind: kind, path: path) }) {
                results.append(summary)
                store.notify(summary)
                store.libraryChanged()
            }
        }
    }
}

/// The short guide Needle shows the first time it opens.
struct Welcome: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    @State private var step = 0

    var body: some View {
        let s = store.settings
        VStack(alignment: .leading, spacing: 16) {
            switch step {
            case 0:
                Text("Welcome to Needle").font(store.display(32))
                Text("Needle plays the music files you own. First, show it where they are. It watches the folder, so new music shows up by itself. Your files are never moved or changed unless you ask.")
                    .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                HStack {
                    Button { store.addFolder() } label: { Label("Choose your music folder…", systemImage: "folder.badge.plus") }
                        .buttonStyle(.borderedProminent)
                    Button("Try three demo songs") { store.demo() }
                }
                if store.scan.running {
                    Text("Reading \(store.scan.scanned.formatted()) songs… \(store.scan.current)").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            case 1:
                Text("Your privacy").font(store.display(28))
                Text("Needle needs no account, and your library stays on this Mac. Choose what may go online. You can change these later in Settings.")
                    .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                Toggle(isOn: Binding(get: { s.onlineMedia }, set: { store.set(["online_media": $0]) })) {
                    VStack(alignment: .leading) {
                        Text("Look up lyrics, covers, and artist photos")
                        Text("Sends only the artist, album, and song title.").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Toggle(isOn: Binding(get: { s.checkUpdates }, set: { store.set(["check_updates": $0]) })) {
                    VStack(alignment: .leading) {
                        Text("Check for new versions")
                        Text("Once a day, asks needle.nnx.fyi for the newest version number.").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Toggle(isOn: Binding(get: { s.discordPresence }, set: { store.set(["discord_presence": $0]) })) {
                    VStack(alignment: .leading) {
                        Text("Show what you play on Discord")
                        Text("While Discord runs on this Mac, your profile shows the song. Finding its cover asks Apple's iTunes catalog by artist and title.").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Toggle(isOn: Binding(get: { s.crashReports }, set: { store.set(["crash_reports": $0]) })) {
                    VStack(alignment: .leading) {
                        Text("Send crash reports")
                        Text("If Needle crashes, it tells us where, without file paths, your name, or anything about your music.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            case 2:
                Text("Choose a look").font(store.display(28))
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 14)], spacing: 14) {
                    ForEach(Palette.bases, id: \.0) { mode, name, about in ThemeCard(mode: mode, name: name, about: about, theme: nil) }
                }
                Text("You can change it, and much more, in Settings › Appearance.").font(.callout).foregroundStyle(.secondary)
            default:
                Text("A few things to know").font(store.display(28))
                tip("command", "⌘K", "Go anywhere, run anything, and find songs, albums, and artists.")
                tip("text.magnifyingglass", "Search", "The search box takes rules too. Save a rule as a playlist that keeps itself up to date.")
                tip("slider.vertical.3", "Sound", "The equalizer button next to the volume opens the equalizer and effects.")
                tip("airplayaudio", "Play on", "Play on Chromecast, AirPlay, and DLNA speakers, one or several at once.")
                tip("iphone", "Phone", "Turn on the phone remote in Settings › Playback to control Needle from your phone.")
                Link("Help and answers: needle.nnx.fyi/help", destination: URL(string: "https://needle.nnx.fyi/help")!).font(.callout)
            }
            Spacer(minLength: 0)
            HStack {
                if step == 0 { Button("Skip the guide") { finish() }.buttonStyle(.link) }
                Spacer()
                if step > 0 { Button("Back") { step -= 1 } }
                Button(step == 3 ? "Start listening" : "Next") { step == 3 ? finish() : (step += 1) }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(28)
        .frame(width: 560, height: 440)
    }

    private func tip(_ icon: String, _ title: String, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: icon).foregroundStyle(store.colors.accent).frame(width: 24)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.callout.weight(.semibold))
                Text(text).font(.callout).foregroundStyle(.secondary)
            }
        }
    }

    private func finish() {
        store.set(["welcomed": true])
        dismiss()
    }
}

/// What a new version of Needle brings.
struct WhatsNew: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let notes: Notes

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("What's new in Needle \(notes.version)").font(store.display(26))
            ScrollView {
                Text((try? AttributedString(markdown: notes.text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(notes.text))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
            HStack {
                Link("Full release notes", destination: URL(string: "https://needle.nnx.fyi/changelog")!)
                Spacer()
                Button("Got it") { dismiss() }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
            }
        }
        .padding(26)
        .frame(width: 560, height: 520)
    }
}

/// A plugin's question: some text to type.
struct PluginQuestion: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    let question: Store.Question
    @State private var answer = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(question.title).font(.title3.weight(.semibold))
            Text("Asked by \(store.plugins.first { $0.id == question.plugin }?.name ?? question.plugin)").font(.caption).foregroundStyle(.secondary)
            if !question.prompt.isEmpty { Text(question.prompt) }
            TextField("", text: $answer).textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Cancel") { store.answer(question, with: nil); dismiss() }.keyboardShortcut(.cancelAction)
                Button("OK") { store.answer(question, with: answer); dismiss() }.buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
            }
        }
        .padding(22)
        .frame(width: 440)
        .onAppear { answer = question.answer }
    }
}
