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
    /// The ones at the foot.
    static let foot: Set<String> = ["lyrics", "speakers", "queue"]

    /// The chosen buttons, known ones only, each once, with Play always there.
    static func chosen(_ list: [String]) -> [String] {
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
        case "favorite": "star"
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
        let list = PlayerButtons.chosen(store.choices.playerButtons)
        let unused = PlayerButtons.all.filter { !list.contains($0) }
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
                Text("Drag to change the order. The middle row, the star, and the foot each keep it.")
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
                Button("Reset") { save(PlayerButtons.all) }
                    .disabled(list == PlayerButtons.all)
            }
        }
        .environment(\.editMode, .constant(.active))
        .navigationTitle("Player buttons")
        .navigationBarTitleDisplayMode(.inline)
        .themed()
    }

    private func save(_ list: [String]) {
        store.updateChoices { $0.playerButtons = PlayerButtons.chosen(list) }
    }
}
