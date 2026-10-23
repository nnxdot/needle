import SwiftUI
import UniformTypeIdentifiers

/// Settings › Appearance, as on the desktop but for a phone: each choice a small picture card
/// (an accent edge and a check when chosen), two to a row, and each switch a tile with a
/// picture of what it does, in groups with a title and a note.
struct AppearanceView: View {
    @Environment(Store.self) private var store
    @State private var addingTheme = false

    private let two = [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)]

    var body: some View {
        let app = store.app
        ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 30) {
                    look(app).id("look")
                    colours(app).id("colours")
                    player(app).id("player")
                    bar(app).id("bar")
                    lyrics(app).id("lyrics")
                    motion(app).id("motion")
                }
                .padding(.horizontal)
                .padding(.vertical, 12)
            }
            #if DEBUG
            // For testing from a shell: -appearance-scroll player|bar|lyrics opens at that group.
            .task {
                guard let group = UserDefaults.standard.string(forKey: "appearance-scroll") else { return }
                try? await Task.sleep(for: .milliseconds(600))
                reader.scrollTo(group, anchor: .top)
            }
            #endif
        }
        .navigationTitle("Appearance")
        .themed()
        .fileImporter(isPresented: $addingTheme, allowedContentTypes: [UTType(filenameExtension: "toml") ?? .plainText, .plainText]) { result in
            guard case .success(let url) = result, let path = store.copyIn(url) else { return }
            Task {
                if let id = await store.ask({ try $0.importTheme(path: path) }) {
                    store.reloadThemes()
                    store.updateApp { $0.theme = id }
                }
            }
        }
    }

    // MARK: Groups

    private func look(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "Look", note: "Colours for the whole app")
            LazyVGrid(columns: two, spacing: 14) {
                ForEach([("night", "Night", "Charcoal grey"), ("midnight", "Midnight", "True black"), ("day", "Day", "Light and airy")], id: \.0) { id, name, about in
                    PickCard(title: name, note: about, chosen: app.theme == id, height: 96) {
                        store.updateApp { $0.theme = id }
                    } picture: { ThemeSketch(colors: ThemeColors.base(id)) }
                }
                ForEach(store.themes, id: \.id) { theme in
                    PickCard(title: theme.name, note: theme.readOnly ? "Comes with Needle" : "Your theme", chosen: app.theme == theme.id, height: 96) {
                        store.updateApp { $0.theme = theme.id }
                    } picture: { ThemeSketch(colors: colors(of: theme)) }
                    .contextMenu {
                        if !theme.readOnly {
                            NavigationLink(value: Route.themeEditor(theme.id)) { Label("Edit \(theme.name)", systemImage: "pencil") }
                        }
                    }
                }
            }
            HStack(spacing: 10) {
                NavigationLink(value: Route.themeEditor(nil)) { Label("New theme", systemImage: "plus") }
                    .buttonStyle(.glass)
                Button("Add a file…", systemImage: "doc.badge.plus") { addingTheme = true }
                    .buttonStyle(.glass)
                if let own = store.themes.first(where: { $0.id == app.theme && !$0.readOnly }) {
                    NavigationLink(value: Route.themeEditor(own.id)) { Label("Edit", systemImage: "pencil") }
                        .buttonStyle(.glass)
                }
            }
            .font(.subheadline)
        }
    }

    private func colours(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "Colours", note: "What tints Needle")
            LazyVGrid(columns: two, spacing: 12) {
                ToggleTile(title: "From the song", detail: "The whole app takes the playing song's colours.", on: app.ambientColors,
                           set: { v in store.updateApp { $0.ambientColors = v } }) { SongTintPicture() }
                ToggleTile(title: "From covers", detail: "Album, playlist, and artist pages take their cover's.", on: app.coverColors,
                           set: { v in store.updateApp { $0.coverColors = v } }) { CoverTintPicture() }
            }
            Tile {
                HStack {
                    ColorPicker(selection: Binding(
                        get: { Color(hex: app.accentColor) ?? ThemeColors.pink },
                        set: { c in store.updateApp { $0.accentColor = c.hex } }), supportsOpacity: false) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Accent").font(.subheadline.weight(.semibold))
                            Text(app.accentColor.isEmpty ? "The theme's own" : "Your colour").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                if !app.accentColor.isEmpty {
                    Button("Use the theme's accent") { store.updateApp { $0.accentColor = "" } }
                        .font(.subheadline)
                        .padding(.top, 4)
                }
            }
        }
    }

    private func player(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "The player", note: "While music plays")
            ToggleTile(title: "Immersive cover", detail: "The cover fills the top of the player, edge to edge, as in Apple Music.",
                       on: app.immersiveCover, set: { v in store.updateApp { $0.immersiveCover = v } }, height: 150) {
                ImmersivePicture(on: app.immersiveCover)
            }
            if !app.immersiveCover {
                Question(title: "The cover") {
                    PickCard(title: "Square", chosen: app.coverShape != "round") {
                        store.updateApp { $0.coverShape = "square" }
                    } picture: { PlayerSketch(part: .square) }
                    PickCard(title: "Record", chosen: app.coverShape == "round") {
                        store.updateApp { $0.coverShape = "round" }
                    } picture: { PlayerSketch(part: .record) }
                }
            }
            Question(title: "Beside play") {
                PickCard(title: "Skip songs", chosen: app.sideButtons != "jump") {
                    store.updateApp { $0.sideButtons = "skip" }
                } picture: { PlayerSketch(part: .skip) }
                PickCard(title: "Jump 10 seconds", chosen: app.sideButtons == "jump") {
                    store.updateApp { $0.sideButtons = "jump" }
                } picture: { PlayerSketch(part: .jump) }
            }
            LazyVGrid(columns: two, spacing: 12) {
                ToggleTile(title: "Cover behind", detail: app.immersiveCover ? "Behind the lyrics and the queue, blurred." : "The player's background, blurred.",
                           on: app.coverBackdrop, set: { v in store.updateApp { $0.coverBackdrop = v } }) { BackdropPicture(on: app.coverBackdrop) }
                ToggleTile(title: "Volume slider", detail: "Under the controls.", on: app.volumeSlider,
                           set: { v in store.updateApp { $0.volumeSlider = v } }) { PlayerSketch(part: .volume) }
                if app.coverBackdrop {
                    ToggleTile(title: "Moving cover", detail: "The blurred cover drifts slowly.", on: app.movingBackdrop,
                               set: { v in store.updateApp { $0.movingBackdrop = v } }) { EmptyView() }
                    Tile {
                        Text("Softness").font(.subheadline.weight(.semibold))
                        Text("\(Int(app.backdropBlur))").font(.caption).foregroundStyle(.secondary).monospacedDigit()
                        Slider(value: Binding(get: { Double(app.backdropBlur) }, set: { v in store.updateApp { $0.backdropBlur = Float(v.rounded()) } }), in: 20...120)
                            .accessibilityLabel("Softness")
                    }
                }
                ToggleTile(title: "Moving covers", detail: "A cover.mp4 beside the songs plays as the cover.", on: app.animatedCovers,
                           set: { v in store.updateApp { $0.animatedCovers = v } }) { EmptyView() }
                ToggleTile(title: "Swipe to skip", detail: "Swipe the cover for the next or last song.", on: app.swipeCover,
                           set: { v in store.updateApp { $0.swipeCover = v } }) { EmptyView() }
                ToggleTile(title: "Open on play", detail: "The player opens when you choose a song.", on: app.openPlayerOnPlay,
                           set: { v in store.updateApp { $0.openPlayerOnPlay = v } }) { EmptyView() }
            }
        }
    }

    private func bar(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "The mini player", note: "And the songs in lists")
            LazyVGrid(columns: two, spacing: 12) {
                ToggleTile(title: "Lyrics under it", detail: "The line being sung, in place of the artist.", on: app.miniPlayerLyrics,
                           set: { v in store.updateApp { $0.miniPlayerLyrics = v } }) { MiniLyricsPicture(on: app.miniPlayerLyrics) }
                ToggleTile(title: "Mark server songs", detail: "A small server icon beside their artist.", on: app.serverMark,
                           set: { v in store.updateApp { $0.serverMark = v } }) { ServerMarkPicture(on: app.serverMark) }
            }
        }
    }

    private func lyrics(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "Lyrics", note: "In the player")
            LazyVGrid(columns: two, spacing: 12) {
                ToggleTile(title: "Live lyrics", detail: "They move with the song, the line playing lit.", on: app.liveLyrics,
                           set: { v in store.updateApp { $0.liveLyrics = v } }) { LiveLyricsPicture(on: app.liveLyrics) }
                ToggleTile(title: "Screen stays on", detail: "While the lyrics show.", on: app.keepScreenOnLyrics,
                           set: { v in store.updateApp { $0.keepScreenOnLyrics = v } }) { EmptyView() }
            }
        }
    }

    private func motion(_ app: AppSettings) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupTitle(title: "Motion and touch", note: "Animations and taps")
            LazyVGrid(columns: two, spacing: 12) {
                ToggleTile(title: "Haptics", detail: "A little tap on play, pause, and long presses.", on: app.haptics,
                           set: { v in store.updateApp { $0.haptics = v } }) { EmptyView() }
                ToggleTile(title: "Reduce motion", detail: "Needle's own switch, besides the iPhone's.", on: app.reduceMotion,
                           set: { v in store.updateApp { $0.reduceMotion = v } }) { EmptyView() }
            }
        }
    }

    /// A theme file's colours, over its base.
    private func colors(of theme: ThemeInfo) -> ThemeColors {
        var c = ThemeColors.base(theme.base)
        if let v = theme.colors["page"].flatMap(Color.init(hex:)) { c.page = v }
        if let v = theme.colors["card"].flatMap(Color.init(hex:)) { c.card = v }
        if let v = theme.colors["text"].flatMap(Color.init(hex:)) { c.text = v }
        if let v = theme.colors["text_muted"].flatMap(Color.init(hex:)) { c.muted = v }
        if let v = theme.colors["accent"].flatMap(Color.init(hex:)) { c.accent = v }
        return c
    }
}

// MARK: - Pieces

/// A group's title, with a short note beside it.
private struct GroupTitle: View {
    let title: String
    let note: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title).font(.title2.bold())
            Text(note).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

/// A question with its two answers side by side.
private struct Question<Cards: View>: View {
    let title: String
    @ViewBuilder let cards: Cards

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            HStack(alignment: .top, spacing: 12) { cards }
        }
    }
}

/// The round accent check on a chosen card's corner.
private struct CheckBadge: View {
    @Environment(Store.self) private var store

    var body: some View {
        Image(systemName: "checkmark")
            .font(.system(size: 10, weight: .heavy))
            .foregroundStyle(.white)
            .frame(width: 22, height: 22)
            .background(store.colors.accent, in: .circle)
            .overlay(Circle().strokeBorder(store.colors.page, lineWidth: 2))
            .accessibilityHidden(true)
    }
}

/// A card's box: glass, with an accent edge when chosen or on.
private struct CardBox: ViewModifier {
    @Environment(Store.self) private var store
    let lit: Bool
    var badge = false

    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: 18, style: .continuous)
        content
            .glassEffect(.regular, in: shape)
            .overlay(shape.strokeBorder(lit ? store.colors.accent : .clear, lineWidth: 2))
            .overlay(alignment: .topTrailing) { if lit && badge { CheckBadge().offset(x: 6, y: -6) } }
            .contentShape(shape)
    }
}

/// A plain box for a setting that is not on or off.
private struct Tile<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 2) { content }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(CardBox(lit: false))
    }
}

/// One answer: a picture and its name; a tap chooses it.
private struct PickCard<Picture: View>: View {
    @Environment(Store.self) private var store
    let title: String
    var note: String?
    let chosen: Bool
    var height: CGFloat = 84
    let action: () -> Void
    @ViewBuilder let picture: Picture

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 8) {
                picture
                    .frame(maxWidth: .infinity)
                    .frame(height: height)
                    .clipShape(.rect(cornerRadius: 11))
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    if let note { Text(note).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                .padding(.horizontal, 3)
            }
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(CardBox(lit: chosen, badge: true))
        }
        .buttonStyle(Pressable())
        .sensoryFeedback(.selection, trigger: chosen) { _, _ in store.app.haptics }
        .accessibilityLabel(note.map { "\(title), \($0)" } ?? title)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// A setting that is on or off: a picture (when it has one), its name, and a switch; a tap
/// anywhere on it turns it over.
private struct ToggleTile<Picture: View>: View {
    @Environment(Store.self) private var store
    let title: String
    let detail: String
    let on: Bool
    let set: (Bool) -> Void
    var height: CGFloat = 70
    @ViewBuilder let picture: Picture

    var body: some View {
        Button { set(!on) } label: {
            VStack(alignment: .leading, spacing: 10) {
                if ObjectIdentifier(Picture.self) != ObjectIdentifier(EmptyView.self) {
                    picture
                        .frame(maxWidth: .infinity)
                        .frame(height: height)
                        .clipShape(.rect(cornerRadius: 11))
                }
                HStack(alignment: .top, spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title).font(.subheadline.weight(.semibold)).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                        Text(detail).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    }
                    Spacer(minLength: 0)
                    // Shown, not tapped: the whole tile is the button. Drawn small, so it stays
                    // inside a narrow tile (the system's switch would not shrink).
                    Capsule()
                        .fill(on ? AnyShapeStyle(store.colors.accent) : AnyShapeStyle(Color.secondary.opacity(0.35)))
                        .frame(width: 38, height: 23)
                        .overlay(alignment: on ? .trailing : .leading) {
                            Circle().fill(.white).padding(2).shadow(color: .black.opacity(0.2), radius: 1, y: 1)
                        }
                        .animation(.snappy(duration: 0.2), value: on)
                        .accessibilityHidden(true)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .modifier(CardBox(lit: on))
        }
        .buttonStyle(Pressable())
        .sensoryFeedback(.selection, trigger: on) { _, _ in store.app.haptics }
        .accessibilityLabel(title)
        .accessibilityValue(on ? "On" : "Off")
        .accessibilityHint(detail)
        .accessibilityAddTraits(.isToggle)
    }
}

// MARK: - Pictures

/// The playing song: its cover, title, and artist (or made-up ones when nothing plays).
private struct Playing {
    let artwork: String?
    let seed: String
    let title: String
    let artist: String

    @MainActor init(_ store: Store) {
        let song = store.playback?.current
        artwork = song?.artwork
        seed = (song?.album ?? "") + (song?.artist ?? "Needle")
        title = song?.title ?? "Your song"
        artist = song?.artist ?? "Its artist"
    }
}

/// Greys for the sketches, and the accent for the one part that changes.
private struct Ink {
    let back: Color
    let part: Color
    let soft: Color
    let accent: Color

    @MainActor init(_ store: Store) {
        let c = store.colors
        back = c.dark ? Color(white: 0.12) : Color(white: 0.9)
        part = c.muted.opacity(0.5)
        soft = c.muted.opacity(0.22)
        accent = c.accent
    }
}

/// A phone in a look's own colours: a title, two rows of cards, and an accent button.
private struct ThemeSketch: View {
    let colors: ThemeColors

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Capsule().fill(colors.text).frame(width: 46, height: 6)
            HStack(spacing: 6) {
                ForEach(0..<3, id: \.self) { _ in RoundedRectangle(cornerRadius: 5).fill(colors.card) }
            }
            .frame(height: 30)
            Capsule().fill(colors.muted.opacity(0.7)).frame(width: 60, height: 4)
            Capsule().fill(colors.accent).frame(width: 40, height: 12)
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(colors.page)
        .accessibilityHidden(true)
    }
}

/// The player with the immersive cover on (the cover fills the top and fades) or off (a cover
/// in the middle), with the playing song's name.
private struct ImmersivePicture: View {
    @Environment(Store.self) private var store
    let on: Bool

    var body: some View {
        let p = Playing(store)
        let page = store.colors.page
        GeometryReader { g in
            ZStack(alignment: .bottomLeading) {
                page
                if on {
                    Artwork(path: p.artwork, seed: p.seed, size: 600)
                        .frame(width: g.size.width, height: g.size.height)
                        .clipped()
                        .overlay {
                            LinearGradient(colors: [page.opacity(0), page.opacity(0.75), page], startPoint: .center, endPoint: .bottom)
                        }
                } else {
                    Artwork(path: p.artwork, seed: p.seed, size: 300)
                        .frame(width: g.size.height * 0.62, height: g.size.height * 0.62)
                        .clipShape(.rect(cornerRadius: 8))
                        .shadow(color: .black.opacity(0.35), radius: 8, y: 4)
                        .frame(width: g.size.width, height: g.size.height * 0.72)
                }
                VStack(alignment: .leading, spacing: 1) {
                    Text(p.title).font(.subheadline.weight(.bold)).lineLimit(1)
                    Text(p.artist).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                .padding(10)
            }
        }
        .accessibilityHidden(true)
    }
}

/// Parts of the player's sketch an answer changes.
private struct PlayerSketch: View {
    @Environment(Store.self) private var store
    enum Part { case square, record, skip, jump, volume }
    let part: Part

    var body: some View {
        let ink = Ink(store)
        VStack(spacing: 7) {
            switch part {
            case .square:
                RoundedRectangle(cornerRadius: 5).fill(ink.accent).frame(width: 46, height: 46)
                controls(ink, accentSides: false)
            case .record:
                Circle().fill(ink.accent).frame(width: 46, height: 46)
                    .overlay(Circle().fill(ink.back).frame(width: 9, height: 9))
                controls(ink, accentSides: false)
            case .skip, .jump:
                RoundedRectangle(cornerRadius: 5).fill(ink.soft).frame(width: 40, height: 40)
                controls(ink, accentSides: true)
            case .volume:
                RoundedRectangle(cornerRadius: 5).fill(ink.soft).frame(width: 34, height: 34)
                controls(ink, accentSides: false)
                HStack(spacing: 5) {
                    Image(systemName: "speaker.fill").font(.system(size: 7))
                    Capsule().fill(ink.accent).frame(height: 3)
                    Image(systemName: "speaker.wave.3.fill").font(.system(size: 7))
                }
                .foregroundStyle(ink.accent)
                .padding(.horizontal, 16)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(ink.back)
        .accessibilityHidden(true)
    }

    private func controls(_ ink: Ink, accentSides: Bool) -> some View {
        let left = part == .jump ? "gobackward.10" : "backward.fill"
        let right = part == .jump ? "goforward.10" : "forward.fill"
        return HStack(spacing: 14) {
            Image(systemName: left).foregroundStyle(accentSides ? ink.accent : ink.part)
            Image(systemName: "play.fill").foregroundStyle(ink.part)
            Image(systemName: right).foregroundStyle(accentSides ? ink.accent : ink.part)
        }
        .font(.system(size: accentSides ? 13 : 10, weight: .bold))
    }
}

/// The player over its cover, blurred, when on.
private struct BackdropPicture: View {
    @Environment(Store.self) private var store
    let on: Bool

    var body: some View {
        let p = Playing(store)
        ZStack {
            if on {
                Artwork(path: p.artwork, seed: p.seed, size: 200).blur(radius: 14).scaleEffect(1.3)
            } else {
                store.colors.page
            }
            Artwork(path: p.artwork, seed: p.seed, size: 200)
                .frame(width: 34, height: 34).clipShape(.rect(cornerRadius: 5))
        }
        .clipped()
        .accessibilityHidden(true)
    }
}

/// The page tinted by the playing cover.
private struct SongTintPicture: View {
    @Environment(Store.self) private var store

    var body: some View {
        let p = Playing(store)
        ZStack(alignment: .topLeading) {
            store.colors.page
            RadialGradient(colors: [(store.coverColor ?? store.colors.accent).opacity(0.6), .clear], center: .top, startRadius: 0, endRadius: 90)
            HStack(spacing: 8) {
                Artwork(path: p.artwork, seed: p.seed, size: 120).frame(width: 30, height: 30).clipShape(.rect(cornerRadius: 5))
                Capsule().fill(store.coverColor ?? store.colors.accent).frame(height: 5)
            }
            .padding(10)
        }
        .accessibilityHidden(true)
    }
}

/// An album page in its cover's colour.
private struct CoverTintPicture: View {
    @Environment(Store.self) private var store

    var body: some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 5).fill(Color(hex: "#5B8C5A")!).frame(width: 30, height: 30)
            VStack(alignment: .leading, spacing: 4) {
                Capsule().fill(.white.opacity(0.85)).frame(width: 46, height: 5)
                Capsule().fill(.white.opacity(0.5)).frame(width: 30, height: 4)
            }
            Spacer(minLength: 0)
        }
        .padding(10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(LinearGradient(colors: [Color(hex: "#3F6B3E")!, store.colors.page], startPoint: .top, endPoint: .bottom))
        .accessibilityHidden(true)
    }
}

/// The mini player with, when on, the line being sung in place of the artist.
private struct MiniLyricsPicture: View {
    @Environment(Store.self) private var store
    let on: Bool

    var body: some View {
        let p = Playing(store)
        HStack(spacing: 7) {
            Artwork(path: p.artwork, seed: p.seed, size: 120).frame(width: 26, height: 26).clipShape(.circle)
            VStack(alignment: .leading, spacing: 1) {
                Text(p.title).font(.caption2.weight(.semibold)).lineLimit(1)
                if on {
                    Text("♪ the line being sung").font(.caption2).italic().lineLimit(1)
                } else {
                    Text(p.artist).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            Image(systemName: "play.fill").font(.caption2)
        }
        .padding(.horizontal, 8)
        .frame(height: 38)
        .glassEffect(.regular, in: .capsule)
        .padding(.horizontal, 6)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(store.colors.page)
        .accessibilityHidden(true)
    }
}

/// Two song rows, with the server mark beside their artist when on.
private struct ServerMarkPicture: View {
    @Environment(Store.self) private var store
    let on: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            ForEach([("Opening", "#C8553D"), ("Low Tide", "#4A6FA5")], id: \.0) { title, hex in
                HStack(spacing: 6) {
                    RoundedRectangle(cornerRadius: 3).fill(Color(hex: hex)!).frame(width: 18, height: 18)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(title).font(.caption2.weight(.semibold))
                        HStack(spacing: 3) {
                            if on { Image(systemName: "server.rack").font(.system(size: 7)) }
                            Text("Mara Quinn").font(.system(size: 8))
                        }
                        .foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(store.colors.page)
        .accessibilityHidden(true)
    }
}

/// Lyrics with the line playing lit (on), or plain words (off).
private struct LiveLyricsPicture: View {
    @Environment(Store.self) private var store
    let on: Bool

    var body: some View {
        let ink = Ink(store)
        VStack(alignment: .leading, spacing: 5) {
            ForEach(0..<3, id: \.self) { i in
                Capsule().fill(on && i == 1 ? ink.accent : ink.part).frame(width: [60.0, 74, 50][i], height: on && i == 1 ? 6 : 4)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(ink.back)
        .accessibilityHidden(true)
    }
}
