import AppKit
import SwiftUI

@main
struct NeedleApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @State private var store = Store.shared

    var body: some Scene {
        Window("Needle", id: "main") {
            MainWindow()
                .environment(store)
                .onAppear { delegate.store = store }
        }
        .defaultSize(width: 1320, height: 860)
        .windowToolbarStyle(.unified)
        .commands { NeedleCommands(store: store) }

        Window("Mini player", id: "mini") {
            MiniPlayer()
                .environment(store)
        }
        .windowStyle(.plain)
        .windowResizability(.contentSize)
        .windowBackgroundDragBehavior(.enabled)
        .defaultPosition(.topTrailing)

        Settings {
            SettingsWindow()
                .environment(store)
        }

        MenuBarExtra(isInserted: Binding(get: { store.settings.tray }, set: { store.set(["tray": $0]) })) {
            TrayMenu().environment(store)
        } label: {
            Image(systemName: store.isPlaying ? "waveform" : "music.note")
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: Store?
    private var keys: Any?

    func applicationDidFinishLaunching(_ notification: Notification) {
        keys = Keys.install { [weak self] in self?.store }
    }

    /// Closing the window leaves Needle open and the music playing, as Music does: the Dock
    /// icon (or the menu bar's, with Needle there) opens it again; ⌘Q quits.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        MainActor.assumeIsolated {
            if let store { store.open(urls) } else {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                    MainActor.assumeIsolated { self?.store?.open(urls) }
                }
            }
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        MainActor.assumeIsolated { store?.quit() }
    }
}

/// The menu bar icon's menu: play, pause, skip, show Needle, or quit.
struct TrayMenu: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        if let p = store.playback, p.hasSong {
            Text("\(p.title) · \(p.artist)")
            Divider()
        }
        Button(store.isPlaying ? "Pause" : "Play") { store.toggle() }
        Button("Next") { store.next() }
        Button("Previous") { store.previous() }
        Divider()
        Button("Show Needle") {
            openWindow(id: "main")
            NSApp.activate(ignoringOtherApps: true)
        }
        Button("Mini player") { openWindow(id: "mini") }
        Divider()
        Button("Quit Needle") { NSApp.terminate(nil) }
    }
}

/// The menu bar's menus, with Needle's keyboard shortcuts.
struct NeedleCommands: Commands {
    let store: Store
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Add Music Folder…") { store.addFolder() }.keyboardShortcut("o")
            Button("Import a Playlist…") { importPlaylist() }
            Button("Check Folders for Changes") { store.rescan() }
        }
        CommandGroup(after: .sidebar) {
            Button(store.settings.showInspector && store.panel == .queue ? "Hide Queue" : "Show Queue") { togglePanel(.queue) }
                .keyboardShortcut("j")
            Button(store.settings.showInspector && store.panel == .lyrics ? "Hide Lyrics" : "Show Lyrics") { togglePanel(.lyrics) }
                .keyboardShortcut("l")
            Button(store.settings.showInspector && store.panel == .details ? "Hide Details" : "Show Details") { togglePanel(.details) }
                .keyboardShortcut("i")
            Divider()
            Button(store.big ? "Close the Big Player" : "Big Player") { store.big.toggle() }.keyboardShortcut("p")
            Button("Full Screen Player") { store.setImmersive(!store.immersive) }.keyboardShortcut("f", modifiers: [.command, .control])
            Button("Mini Player") { openWindow(id: "mini") }.keyboardShortcut("m", modifiers: [.command, .option])
            Divider()
            Button("Command Palette") { store.palette.toggle() }.keyboardShortcut("k")
            Button("Search") { store.focusSearch += 1 }.keyboardShortcut("f")
        }
        CommandMenu("Controls") {
            Button(store.isPlaying ? "Pause" : "Play") { store.toggle() }
            Button("Next") { store.next() }.keyboardShortcut(.rightArrow)
            Button("Previous") { store.previous() }.keyboardShortcut(.leftArrow)
            Button("Forward 10 Seconds") { store.seekBy(10) }.keyboardShortcut(.rightArrow, modifiers: [.command, .option])
            Button("Back 10 Seconds") { store.seekBy(-10) }.keyboardShortcut(.leftArrow, modifiers: [.command, .option])
            Divider()
            Button("Volume Up") { store.nudgeVolume(0.05) }.keyboardShortcut(.upArrow)
            Button("Volume Down") { store.nudgeVolume(-0.05) }.keyboardShortcut(.downArrow)
            Button(store.mutedVolume == nil ? "Mute" : "Unmute") { store.toggleMute() }
            Divider()
            Button("Shuffle Up Next") { store.core.shuffle(); store.notify("Up next shuffled.") }
            Button(repeatLabel) { store.cycleRepeat() }.keyboardShortcut("r")
            Button("A–B Loop") { store.loopButton() }
            Divider()
            Button("Clear the Queue") { store.core.clearQueue() }
            if let current = store.current {
                Button("Start Radio from This Song") { store.startRadio(current) }
                Button(current.favorite ? "Remove from Favorites" : "Add to Favorites") { store.toggleFavorite([current]) }
                    .keyboardShortcut("d", modifiers: [.command, .shift])
                Button("Time the Lyrics") { store.navigate(.timing) }
            }
        }
        CommandMenu("Song") {
            let chosen = store.selectedTracks
            Button(chosen.count > 1 ? "Play \(chosen.count) Tracks" : "Play") { store.play(chosen) }.disabled(chosen.isEmpty)
            Button("Play Next") { store.playNext(chosen.map(\.id)) }.disabled(chosen.isEmpty)
            Button("Add to Queue") { store.enqueue(chosen.map(\.id)) }.disabled(chosen.isEmpty)
            Divider()
            Button(chosen.allSatisfy(\.favorite) && !chosen.isEmpty ? "Remove from Favorites" : "Add to Favorites") { store.toggleFavorite(chosen) }
                .keyboardShortcut("d")
                .disabled(chosen.isEmpty)
            Button("Edit Tags…") { store.tagging = chosen.map(\.id) }
                .keyboardShortcut("e")
                .disabled(chosen.isEmpty || chosen.contains(where: \.streamed))
            Divider()
            if chosen.count == 1, let song = chosen.first {
                Button("Go to Album") { store.navigate(song.albumRoute) }
                Button("Go to Artist") { store.navigate(.artist(song.artist)) }
                Button("Start Radio") { store.startRadio(song) }
                if !song.streamed {
                    Button("Show in Finder") { store.reveal(song.filePath) }.keyboardShortcut("r", modifiers: [.command, .shift])
                }
            }
        }
        CommandMenu("Go") {
            Button("Back") { store.goBack() }.keyboardShortcut("[")
            Button("Forward") { store.goForward() }.keyboardShortcut("]")
            Divider()
            Button("Home") { store.navigate(.home) }.keyboardShortcut("1")
            Button("Songs") { store.navigate(.songs) }.keyboardShortcut("2")
            Button("Albums") { store.navigate(.albums) }.keyboardShortcut("3")
            Button("Artists") { store.navigate(.artists) }.keyboardShortcut("4")
            Button("Favorites") { store.navigate(.favorites) }.keyboardShortcut("5")
            Button("Recently Added") { store.navigate(.recent) }.keyboardShortcut("6")
            Button("Listening History") { store.navigate(.history) }.keyboardShortcut("7")
            Button("Folders") { store.navigate(.folders) }
            Divider()
            if let current = store.current {
                Button("Go to the Album Playing") { store.navigate(current.albumRoute) }
                Button("Go to the Artist Playing") { store.navigate(.artist(current.artist)) }
                Divider()
            }
            Button("Sound and Equalizer") { store.navigate(.sound) }
            Button("Fix My Library") { store.navigate(.doctor) }
            Button("Import from Other Apps") { store.navigate(.importing) }
            Button("Your Year in Music") { store.navigate(.wrapped(Calendar.current.component(.year, from: Date()))) }
        }
        CommandGroup(replacing: .help) {
            Button("Needle Help") { NSWorkspace.shared.open(URL(string: "https://needle.nnx.fyi/help")!) }
            Button("What's New") { store.whatsNew = store.core.desktopWhatsNew().first }
            Button("Welcome Guide") { store.welcome = true }
        }
    }

    private var repeatLabel: String {
        switch store.playback?.repeat {
        case "all": "Repeat: All"
        case "one": "Repeat: One"
        default: "Repeat: Off"
        }
    }

    private func togglePanel(_ panel: Store.Panel) {
        if store.settings.showInspector && store.panel == panel {
            store.set(["show_inspector": false])
        } else {
            store.panel = panel
            store.set(["show_inspector": true])
        }
    }

    private func importPlaylist() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.init(filenameExtension: "m3u")!, .init(filenameExtension: "m3u8")!]
        guard panel.runModal() == .OK, let path = panel.url?.path else { return }
        store.job { try $0.importPlaylistFile(path: path) }
    }
}
