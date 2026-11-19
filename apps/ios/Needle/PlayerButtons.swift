import SwiftUI

/// The player's buttons you choose (Settings › Appearance › Player buttons): the library's
/// `player_buttons`, in order. In the player, Shuffle, Previous, Play, Next, and Repeat sit in
/// its middle row, Favorite beside the song's name, and Lyrics, Speakers, and Queue at its foot,
/// each in the order chosen.
enum PlayerButtons {
    /// Every button, in Needle's own order.
    static let all = ["shuffle", "previous", "play", "next", "repeat", "favorite", "lyrics", "queue", "speakers"]
    /// The ones in the middle row.
    static let transport: Set<String> = ["shuffle", "previous", "play", "next", "repeat"]
    /// The ones at the foot (the player before 2.0.7; its pill of tools is fixed now).
    static let foot: Set<String> = ["lyrics", "speakers", "queue"]
    /// The ones the player obeys: the main row and the heart beside the song's name. The others
    /// stay in the saved list, in their places, for an older or a later version.
    static let shown: Set<String> = transport.union(["favorite"])
    /// Those, in Needle's own order: what Settings shows and counts.
    static let editable = all.filter { shown.contains($0) }

    /// `full` with the shown buttons in `order` (in the shown ones' places); a hidden button
    /// keeps its place, and one added goes at the end.
    static func merged(_ full: [String], order: [String]) -> [String] {
        var rest = order[...]
        var result: [String] = []
        for name in full {
            if shown.contains(name) {
                if let next = rest.popFirst() { result.append(next) }
            } else {
                result.append(name)
            }
        }
        result.append(contentsOf: rest)
        return result
    }
    /// The iPhone's own first choice, as Apple Music's: Previous, Play, and Next in the middle,
    /// no Shuffle or Repeat (they are in Up Next).
    static let phoneDefault = all.filter { $0 != "shuffle" && $0 != "repeat" }

    /// The chosen buttons, known ones only, each once, with Play always there. The core's
    /// untouched default (every button, for the computers) is the iPhone's own default here.
    static func chosen(_ list: [String]) -> [String] {
        if list == all { return phoneDefault }
        var seen: [String] = []
        for name in list where all.contains(name) && !seen.contains(name) { seen.append(name) }
        if !seen.contains("play") { seen.insert("play", at: 0) }
        return seen
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
        default: "airplayaudio"
        }
    }
}

/// Settings › Appearance › Player buttons: the ones in the player, to drag into order or take
/// out (Play/Pause always stays), and the ones not in it, to add.
struct PlayerButtonsView: View {
    @Environment(Store.self) private var store

    var body: some View {
        let full = PlayerButtons.chosen(store.choices.playerButtons)
        let list = full.filter { PlayerButtons.shown.contains($0) }
        let unused = PlayerButtons.editable.filter { !list.contains($0) }
        List {
            Section {
                ForEach(list, id: \.self) { name in
                    Label(PlayerButtons.name(name), systemImage: PlayerButtons.symbol(name))
                        .deleteDisabled(name == "play")
                }
                .onMove { from, to in
                    var next = list
                    next.move(fromOffsets: from, toOffset: to)
                    save(next)
                }
                .onDelete { at in
                    save(list.enumerated().filter { !at.contains($0.offset) || $0.element == "play" }.map(\.element))
                }
            } header: {
                Text("In the player")
            } footer: {
                Text("Drag to change the order of the middle row. Lyrics, Up Next, Play On, and the sleep timer are always in the pill under it.")
            }
            if !unused.isEmpty {
                Section("Not in the player") {
                    ForEach(unused, id: \.self) { name in
                        Button { save(list + [name]) } label: {
                            HStack {
                                Label(PlayerButtons.name(name), systemImage: PlayerButtons.symbol(name))
                                Spacer()
                                Image(systemName: "plus.circle.fill").foregroundStyle(.green)
                            }
                        }
                        .foregroundStyle(.primary)
                        .accessibilityLabel("Add \(PlayerButtons.name(name))")
                    }
                }
            }
            Section {
                Button("Reset") { store.updateChoices { $0.playerButtons = PlayerButtons.all } }
                    .disabled(list == PlayerButtons.phoneDefault.filter { PlayerButtons.shown.contains($0) })
            }
        }
        .environment(\.editMode, .constant(.active))
        .navigationTitle("Player buttons")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
    }

    /// Saves the shown buttons in this order; the hidden ones keep their places.
    private func save(_ list: [String]) {
        let full = PlayerButtons.chosen(store.choices.playerButtons)
        store.updateChoices { $0.playerButtons = PlayerButtons.chosen(PlayerButtons.merged(full, order: list)) }
    }
}
