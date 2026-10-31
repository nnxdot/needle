import SwiftUI

/// The player's buttons you choose (Settings › Appearance › Layout › Player buttons): the
/// settings' `player_buttons`, in order, as on Windows. Those up to the last of Previous, Play,
/// and Next sit before the song's screen, the rest after it; every button not in the bar waits
/// in the ••• menu at its end.
enum PlayerButtons {
    /// The core's buttons for every system, in Needle's own order (`model::PLAYER_BUTTONS`),
    /// and the settings' list until it is changed.
    static let core = ["shuffle", "previous", "play", "next", "repeat", "favorite", "lyrics", "queue", "speakers"]
    /// Every button the Mac's player has: the core's, then the computers' (`MORE_BUTTONS`).
    static let all = core + ["volume", "sleep", "speed", "loop", "sound", "mini"]
    /// The bar out of the box, as on Windows: the rest are in the ••• menu.
    static let short = ["previous", "play", "next", "favorite", "lyrics", "queue", "volume"]

    /// The chosen buttons, known ones only, each once, with Play always there. A list never
    /// changed (the core's whole list, which every Needle before 2.0 kept) is the short one.
    static func chosen(_ list: [String]) -> [String] {
        if list == core { return short }
        var seen: [String] = []
        for name in list where all.contains(name) && !seen.contains(name) { seen.append(name) }
        if !seen.contains("play") { seen.insert("play", at: 0) }
        return seen
    }

    /// The buttons before the screen, and after it, of a list already chosen.
    static func split(_ list: [String]) -> (before: [String], after: [String]) {
        let cut = (list.lastIndex { ["previous", "play", "next"].contains($0) } ?? 0) + 1
        return (Array(list[..<cut]), Array(list[cut...]))
    }

    static func name(_ button: String) -> String {
        switch button {
        case "shuffle": "Shuffle"
        case "previous": "Previous"
        case "play": "Play/Pause"
        case "next": "Next"
        case "repeat": "Repeat"
        case "favorite": "Favorite"
        case "lyrics": "Lyrics"
        case "queue": "Queue"
        case "volume": "Volume"
        case "sleep": "Sleep timer"
        case "speed": "Speed and pitch"
        case "loop": "A–B loop"
        case "sound": "Equalizer and sound tools"
        case "mini": "Mini player"
        default: "Speakers"
        }
    }

    static func symbol(_ button: String) -> String {
        switch button {
        case "shuffle": "shuffle"
        case "previous": "backward.fill"
        case "play": "playpause.fill"
        case "next": "forward.fill"
        case "repeat": "repeat"
        case "favorite": "heart"
        case "lyrics": "quote.bubble"
        case "queue": "list.bullet"
        case "volume": "speaker.wave.2.fill"
        case "sleep": "moon"
        case "speed": "gauge.with.dots.needle.67percent"
        case "loop": "arrow.left.and.right"
        case "sound": "slider.vertical.3"
        case "mini": "pip"
        default: "airplayaudio"
        }
    }
}

/// One of the player's buttons, as the player bar draws it.
struct PlayerButton: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    let name: String
    /// The play sign's size.
    var play: CGFloat = 30
    /// The volume's slider width (none: only its button).
    var volumeWidth: CGFloat = 64

    var body: some View {
        let colors = store.colors
        let playing = store.playback?.playing == true
        let repeatMode = store.playback?.repeat ?? "off"
        switch name {
        case "shuffle":
            Button { store.shuffleView() } label: { Image(systemName: "shuffle") }
                .buttonStyle(.borderless)
                .help(store.route.isSongs ? "Shuffle this view" : "Shuffle up next")
                .accessibilityLabel("Shuffle")
        case "previous":
            Button { store.previous() } label: { Image(systemName: "backward.fill") }
                .buttonStyle(.borderless)
                .help("Previous · ⌘←")
                .accessibilityLabel("Previous")
        case "play":
            Button { store.toggle() } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.system(size: play * 0.7))
                    .frame(width: play, height: play)
                    .contentShape(Rectangle())
                    .contentTransition(.symbolEffect(.replace))
            }
            .buttonStyle(.borderless)
            .help(playing ? "Pause · Space" : "Play · Space")
            .accessibilityLabel(playing ? "Pause" : "Play")
        case "next":
            Button { store.next() } label: { Image(systemName: "forward.fill") }
                .buttonStyle(.borderless)
                .help("Next · ⌘→")
                .accessibilityLabel("Next")
        case "repeat":
            Button { store.cycleRepeat() } label: {
                Image(systemName: repeatMode == "one" ? "repeat.1" : "repeat")
                    .foregroundStyle(repeatMode == "off" ? colors.ink2 : colors.accent)
            }
            .buttonStyle(.borderless)
            .help(repeatMode == "off" ? "Repeat is off" : repeatMode == "all" ? "Repeating the queue" : "Repeating this track")
            .accessibilityLabel("Repeat")
            .accessibilityValue(repeatMode == "off" ? "Off" : repeatMode == "all" ? "All" : "One")
        case "favorite":
            // Always there (dimmed with nothing playing), so the bar keeps its width.
            Heart(on: store.current?.favorite ?? false, size: 14) {
                if let c = store.current { store.toggleFavorite([c]) }
            }
            .disabled(store.current == nil)
        case "lyrics": PanelButton(panel: .lyrics, icon: "quote.bubble", help: "Lyrics · ⌘L")
        case "queue": QueueButton()
        case "speakers": SpeakersButton()
        case "volume": VolumeControl(width: volumeWidth)
        case "sleep": SleepButton()
        case "speed": SpeedButton()
        case "loop":
            let looping = store.playback?.loopStart != nil
            Button { store.loopButton() } label: {
                Image(systemName: "arrow.left.and.right").foregroundStyle(looping || store.loopStart != nil ? colors.accent : colors.ink2)
            }
            .buttonStyle(.borderless)
            .disabled(store.playback?.hasSong != true)
            .help(looping ? "Clear the A–B loop" : store.loopStart != nil ? "Set point B" : "A–B loop: set point A")
            .accessibilityLabel("A–B loop")
        case "sound":
            Button { store.navigate(.sound) } label: { Image(systemName: "slider.vertical.3").foregroundStyle(colors.ink2) }
                .buttonStyle(.borderless)
                .help("Equalizer and sound tools")
                .accessibilityLabel("Equalizer and sound tools")
        case "mini":
            Button { openWindow(id: "mini") } label: { Image(systemName: "pip").foregroundStyle(colors.ink2) }
                .buttonStyle(.borderless)
                .help("Mini player · ⌥⌘M")
                .accessibilityLabel("Mini player")
        default: EmptyView()
        }
    }
}

/// Player buttons, as a row that opens: a small copy of the player bar to drag buttons into,
/// out of, and along, and the buttons not in it on one line below. A click on one also takes
/// it out or puts it at the end.
struct PlayerButtonsEditor: View {
    private var store: Store { .shared }

    var body: some View {
        let list = PlayerButtons.chosen(store.settings.playerButtons)
        let (before, after) = PlayerButtons.split(list)
        let unused = PlayerButtons.all.filter { !list.contains($0) }
        VStack(alignment: .leading, spacing: 12) {
            // The bar.
            HStack(spacing: 6) {
                ForEach(before, id: \.self) { slot($0, in: list) }
                RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .fill(Color.primary.opacity(0.07))
                    .frame(width: 120, height: 26)
                    .overlay(Text("Song").font(.needle(.caption2)).foregroundStyle(.tertiary))
                    .dropDestination(for: String.self) { names, _ in
                        guard let name = names.first else { return false }
                        save(insert(name, at: before.count, into: list))
                        return true
                    }
                    .accessibilityHidden(true)
                ForEach(after, id: \.self) { slot($0, in: list) }
                Image(systemName: "ellipsis").foregroundStyle(.tertiary).frame(width: 26)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Capsule().fill(Color.primary.opacity(0.05)))
            .overlay(Capsule().strokeBorder(Color.primary.opacity(0.1)))
            .dropDestination(for: String.self) { names, _ in
                guard let name = names.first else { return false }
                save(insert(name, at: list.count, into: list))
                return true
            }
            // The buttons not in the bar.
            HStack(spacing: 6) {
                Text(unused.isEmpty ? "Every button is in the bar." : "Not in the bar:")
                    .font(.needle(.callout)).foregroundStyle(.secondary)
                ForEach(unused, id: \.self) { name in
                    icon(name)
                        .onTapGesture { save(list + [name]) }
                        .draggable(name)
                        .help("Add \(PlayerButtons.name(name))")
                        .accessibilityElement()
                        .accessibilityLabel("Add \(PlayerButtons.name(name))")
                        .accessibilityAddTraits(.isButton)
                        .accessibilityAction { save(list + [name]) }
                }
                Spacer()
                Button("Reset") { save(PlayerButtons.short) }
                    .disabled(list == PlayerButtons.short)
            }
            .frame(minHeight: 30)
            .dropDestination(for: String.self) { names, _ in
                guard let name = names.first, name != "play" else { return false }
                save(list.filter { $0 != name })
                return true
            }
            Text("Drag buttons along the bar, or out of it. Play/Pause always stays.")
                .font(.needle(.caption)).foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
    }

    /// A button in the bar: dragged along it or out of it; a click takes it out.
    private func slot(_ name: String, in list: [String]) -> some View {
        icon(name, lit: true)
            .onTapGesture { if name != "play" { save(list.filter { $0 != name }) } }
            .draggable(name)
            .dropDestination(for: String.self) { names, _ in
                guard let moved = names.first, moved != name else { return false }
                save(insert(moved, at: list.firstIndex(of: name) ?? list.count, into: list))
                return true
            }
            .help(name == "play" ? "Play/Pause always stays" : "Drag to move · click to take out")
            .accessibilityElement()
            .accessibilityLabel(PlayerButtons.name(name))
            .accessibilityHint(name == "play" ? "" : "In the player bar")
            .accessibilityAction(named: "Move left") { move(name, by: -1, in: list) }
            .accessibilityAction(named: "Move right") { move(name, by: 1, in: list) }
            .accessibilityAction(named: "Take out") { if name != "play" { save(list.filter { $0 != name }) } }
    }

    private func icon(_ name: String, lit: Bool = false) -> some View {
        Image(systemName: PlayerButtons.symbol(name))
            .font(.system(size: 13))
            .foregroundStyle(lit ? Color.primary : .secondary)
            .frame(width: 26, height: 26)
            .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Color.primary.opacity(lit ? 0 : 0.06)))
            .contentShape(Rectangle())
    }

    /// `list` with `name` at `index` (moved there when it is in the list already).
    private func insert(_ name: String, at index: Int, into list: [String]) -> [String] {
        var next = list
        var at = index
        if let old = next.firstIndex(of: name) {
            next.remove(at: old)
            if old < at { at -= 1 }
        }
        next.insert(name, at: min(max(0, at), next.count))
        return next
    }

    private func move(_ name: String, by step: Int, in list: [String]) {
        guard let i = list.firstIndex(of: name) else { return }
        let to = min(max(0, i + step), list.count - 1)
        var next = list
        next.swapAt(i, to)
        save(next)
    }

    private func save(_ list: [String]) {
        store.set(["player_buttons": PlayerButtons.chosen(list)])
    }
}
