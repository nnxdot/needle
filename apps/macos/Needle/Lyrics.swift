import SwiftUI
import Translation

/// The playing song's lyrics. Timed ones follow the song, the line sung lit, word by word when
/// the words are timed; click a line to go to it.
struct LyricsView: View {
    private var store: Store { .shared }
    /// `stage`: the big player's Lyrics look, only the words, very large.
    enum Kind { case side, big, mini, immersive, stage }
    let kind: Kind
    var light = false
    /// A line chosen to share as a picture.
    @State private var card: CardLine?
    /// Each line in your language, under it, translated on this Mac (Apple's Translation).
    @AppStorage("translateLyrics") private var translate = false
    @State private var translations: [Int: String] = [:]
    @State private var translation: TranslationSession.Configuration?

    private var size: CGFloat {
        switch kind {
        case .side: 17
        case .big: 26
        case .mini: 15
        case .immersive: 34
        case .stage: 52
        }
    }

    var body: some View {
        let colors = store.colors
        let ink: Color = light ? .white : colors.ink
        Group {
            if store.current == nil {
                note("Play something to see its lyrics.")
            } else if store.lyricsLooking && store.lyrics == nil {
                ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let lyrics = store.lyrics {
                if lyrics.instrumental {
                    note("This song is instrumental.")
                } else if !lyrics.lines.isEmpty {
                    timed(lyrics, ink: ink)
                } else {
                    ScrollView {
                        Text(lyrics.plain)
                            .font(.system(size: size * 0.85, weight: .medium))
                            .foregroundStyle(ink.opacity(0.9))
                            .lineSpacing(6)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: kind == .side || kind == .mini ? .leading : .center)
                            .padding(.vertical, 20)
                        footer(lyrics)
                    }
                }
            } else {
                missing
            }
        }
        .sheet(item: $card) { LyricCardSheet(line: $0.text) }
        .onChange(of: translate) { _, _ in askTranslation() }
        .onChange(of: store.lyrics?.lines.count) { _, _ in askTranslation() }
        .onChange(of: store.current?.id) { _, _ in askTranslation() }
        .onAppear { askTranslation() }
        .translationTask(translation) { session in
            let lines = store.lyrics?.lines ?? []
            let requests = lines.enumerated().compactMap { i, line in
                line.text.trimmingCharacters(in: .whitespaces).isEmpty ? nil : TranslationSession.Request(sourceText: line.text, clientIdentifier: String(i))
            }
            do {
                var found: [Int: String] = [:]
                for response in try await session.translations(from: requests) {
                    // A line already in your language is left alone.
                    if let i = response.clientIdentifier.flatMap(Int.init), response.targetText != response.sourceText {
                        found[i] = response.targetText
                    }
                }
                translations = found
                if found.isEmpty { store.notify("These lyrics are already in your language") }
            } catch {
                store.notify("These lyrics can't be translated on this Mac")
            }
        }
    }

    private func note(_ text: String) -> some View {
        Text(text).foregroundStyle(light ? .white.opacity(0.7) : .secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var missing: some View {
        VStack(spacing: 10) {
            Text("No lyrics for this song").font(.needle(.headline)).foregroundStyle(light ? .white : store.colors.ink)
            if !store.settings.onlineMedia {
                Text("Turn on Look things up online in Settings › Library to find lyrics on LRCLIB.")
                    .font(.needle(.callout)).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            HStack {
                if !store.plugins.contains(where: { $0.id == "netease-lyrics" && $0.enabled }) {
                    Button("Look on NetEase") {
                        guard let id = store.playback?.current else { return }
                        store.run { try $0.addNetease(song: id) }
                        store.notify("NetEase lyrics is on. Looking for lyrics there…")
                        Task {
                            try? await Task.sleep(for: .milliseconds(1500))
                            store.reloadPlugins()
                            store.reloadLyrics()
                        }
                    }
                }
                // The timing page is in the main window: not offered in the mini player.
                if kind != .mini {
                    Button("Write and time them") { store.big = false; store.navigate(.timing) }
                }
            }
            .controlSize(.small)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func timed(_ lyrics: Lyrics, ink: Color) -> some View {
        // Only the line being sung follows the clock (see `SungLine`); the others redraw
        // when the line changes, not ten times a second.
        let line = store.lyricLine
        let center = kind == .big || kind == .immersive || kind == .stage
        return ScrollViewReader { reader in
            ScrollView {
                LazyVStack(alignment: center ? .center : .leading, spacing: kind == .side || kind == .mini ? 12 : kind == .stage ? 28 : 18) {
                    Color.clear.frame(height: kind == .mini ? 20 : 60)
                    ForEach(Array(lyrics.lines.enumerated()), id: \.offset) { index, l in
                        let state: LyricLineView.State = index == line ? .sung : (line.map { index < $0 } ?? false) ? .past : .ahead
                        VStack(alignment: center ? .center : .leading, spacing: 4) {
                            if state == .sung, !l.words.isEmpty {
                                SungLine(line: l, next: index + 1 < lyrics.lines.count ? lyrics.lines[index + 1].time : nil,
                                         size: size, ink: ink, center: center)
                            } else {
                                LyricLineView(text: l.text, state: state, size: size, ink: ink, center: center, motion: store.motion)
                                    .equatable()
                            }
                            // The line in your language, under it (Translate on).
                            if translate, let translated = translations[index] {
                                Text(translated)
                                    .font(.system(size: size * 0.66, weight: .semibold))
                                    .foregroundStyle(ink.opacity(state == .sung ? 0.7 : 0.4))
                                    .multilineTextAlignment(center ? .center : .leading)
                            }
                        }
                        .id(index)
                        .onTapGesture { store.seek(l.time) }
                        .contextMenu {
                            if !l.text.trimmingCharacters(in: .whitespaces).isEmpty {
                                Button("Share as Card…", systemImage: "square.and.arrow.up") { card = CardLine(text: l.text) }
                            }
                            Button(translate ? "Hide Translation" : "Translate Lyrics", systemImage: "translate") { translate.toggle() }
                        }
                    }
                    footer(lyrics)
                    Color.clear.frame(height: kind == .mini ? 40 : 160)
                }
                .frame(maxWidth: .infinity, alignment: center ? .center : .leading)
            }
            .scrollIndicators(.never)
            .onChange(of: line) { _, now in
                guard let now else { return }
                withAnimation(store.motion ? .smooth(duration: 0.6) : nil) { reader.scrollTo(now, anchor: UnitPoint(x: 0.5, y: 0.38)) }
            }
            .onAppear { if let line { reader.scrollTo(line, anchor: UnitPoint(x: 0.5, y: 0.38)) } }
        }
    }

    /// Asks Apple's Translation for these lyrics, into your language, while Translate is on.
    private func askTranslation() {
        translations = [:]
        guard translate, let lines = store.lyrics?.lines, !lines.isEmpty else { translation = nil; return }
        if translation == nil { translation = TranslationSession.Configuration() } else { translation?.invalidate() }
    }

    @ViewBuilder
    private func footer(_ lyrics: Lyrics) -> some View {
        if kind == .side || kind == .big {
            HStack(spacing: 10) {
                Text(source(lyrics.source)).font(.needle(.caption)).foregroundStyle(light ? .white.opacity(0.6) : .secondary)
                Button(translate ? "Hide translation" : "Translate") { translate.toggle() }
                    .buttonStyle(.link).font(.needle(.caption))
                Button("Time these lyrics") { store.big = false; store.navigate(.timing) }
                    .buttonStyle(.link).font(.needle(.caption))
            }
            .padding(.top, 20)
        }
    }

    private func source(_ source: String) -> String {
        switch source {
        case "file": "From the song's own file"
        case "online": "From LRCLIB"
        case "plugin": "From a lyrics plugin"
        default: "Timed in Needle"
        }
    }
}

/// A lyric line not being sung, or being sung without word timings: lit, faded, or dim.
struct LyricLineView: View, Equatable {
    enum State { case past, sung, ahead }
    let text: String
    let state: State
    let size: CGFloat
    let ink: Color
    let center: Bool
    let motion: Bool

    var body: some View {
        Text(text.isEmpty ? "♪" : text)
            .font(.system(size: size, weight: .bold))
            .foregroundStyle(ink.opacity(state == .sung ? 1 : state == .past ? 0.42 : 0.3))
            .multilineTextAlignment(center ? .center : .leading)
            .scaleEffect(state == .sung && motion ? 1.02 : 1, anchor: center ? .center : .leading)
            .animation(motion ? .smooth(duration: 0.3) : nil, value: state)
            .contentShape(Rectangle())
    }
}

/// The line being sung, karaoke style: the words sung so far lit, the one being sung filling.
/// It alone reads the clock.
struct SungLine: View {
    private var store: Store { .shared }
    let line: LyricLine
    let next: Double?
    let size: CGFloat
    let ink: Color
    let center: Bool

    var body: some View {
        let position = (store.playback?.position ?? 0) + 0.15
        var text = AttributedString()
        for (i, w) in line.words.enumerated() {
            let end = i + 1 < line.words.count ? line.words[i + 1].time : (next ?? w.time + 0.6)
            let lit = position >= end ? 1 : position <= w.time ? 0 : (position - w.time) / max(0.05, end - w.time)
            var word = AttributedString(w.text)
            word.foregroundColor = ink.opacity(0.35 + 0.65 * lit)
            text += word
        }
        return Text(text)
            .font(.system(size: size, weight: .bold))
            .multilineTextAlignment(center ? .center : .leading)
            .scaleEffect(store.motion ? 1.02 : 1, anchor: center ? .center : .leading)
            .contentShape(Rectangle())
    }
}

/// Time the lyrics: tap Space as each line (or word) starts while the song plays; −0.2 s and
/// +0.2 s move the line last tapped, Redo line taps it again. The steps are the core's
/// (`LyricTimer`); saved as a .lrc file beside the song.
struct TimingPage: View {
    private var store: Store { .shared }
    /// Words written here, for a song without lyrics.
    @State private var text = ""
    @State private var timer: LyricTimer?
    /// The song being timed.
    @State private var song: String?
    @State private var lines: [TimingLine] = []
    @State private var next = (line: 0, word: 0)
    @State private var selected: Int?
    @State private var words = false
    /// Something changed since it opened or was saved.
    @State private var dirty = false
    @FocusState private var tapping: Bool

    private var playingThis: Bool { song != nil && store.current?.id == song }

    var body: some View {
        let colors = store.colors
        VStack(alignment: .leading, spacing: 14) {
            header
            if store.current == nil && timer == nil {
                Text("Play a song first, then time its lyrics.").foregroundStyle(.secondary)
            } else if store.current?.cue != nil && timer == nil {
                Text("This song is part of a CUE sheet, so it has no file of its own to put lyrics beside.").foregroundStyle(.secondary)
            } else if timer == nil {
                Text("This song has no lyrics yet. Write them here, then tap along to time them.").foregroundStyle(colors.ink2)
                TextEditor(text: $text)
                    .font(.system(size: 14))
                    .scrollContentBackground(.hidden)
                    .padding(8)
                    .background(colors.raised, in: RoundedRectangle(cornerRadius: 10))
                Button { begin(LyricTimer(lyrics: nil, text: text)) } label: { Label("Start timing", systemImage: "play.fill") }
                    .glassButton(prominent: true)
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            } else {
                Text(!playingThis ? "Play this song, then tap along."
                     : words ? "Press Space as each word starts. Backspace undoes. ← and → move the last line by 0.2 s."
                     : "Press Space as each line starts. Backspace undoes. ← and → move the last line by 0.2 s.")
                    .foregroundStyle(colors.ink2)
                list
                tapButton
                fixes
            }
        }
        .padding(28)
        .frame(maxWidth: 820, maxHeight: .infinity, alignment: .topLeading)
        .frame(maxWidth: .infinity)
        .focusable(timer != nil)
        .focusEffectDisabled()
        .focused($tapping)
        .onKeyPress(.space) { guard timer != nil else { return .ignored }; tap(); return .handled }
        .onKeyPress(.delete) { guard let timer, timer.undo() else { return .ignored }; changed(); return .handled }
        .onKeyPress(.leftArrow) { guard let timer, selected != nil else { return .ignored }; timer.earlier(); changed(); return .handled }
        .onKeyPress(.rightArrow) { guard let timer, selected != nil else { return .ignored }; timer.later(); changed(); return .handled }
        .task(id: "\(store.current?.id ?? "")|\(store.lyricsFor ?? "")") { open() }
    }

    private var header: some View {
        let colors = store.colors
        let title = store.current.map { "\($0.title) · \($0.artist)" } ?? ""
        return HStack(alignment: .bottom, spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Time the lyrics").font(store.display(28))
                if !title.isEmpty { Text(title).foregroundStyle(colors.ink2).lineLimit(1) }
            }
            Spacer()
            if timer != nil {
                Picker("", selection: Binding(get: { words }, set: { on in timer?.setWordMode(on: on); changed() })) {
                    Text("Lines").tag(false)
                    Text("Words").tag(true)
                }
                .pickerStyle(.segmented)
                .frame(width: 160)
                Button("Save") { save() }.glassButton(prominent: true).disabled(!dirty)
            }
            Button("Close") { store.goBack() }.glassButton()
        }
    }

    private var list: some View {
        let colors = store.colors
        let position = playingThis ? store.playback?.position : nil
        return ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                        let isNext = i == next.line
                        // Now singing: this line started and the next has not.
                        let now = position.map { pos in
                            (line.time.map { $0 <= pos } ?? false)
                                && (i + 1 >= lines.count || lines[i + 1].time.map { pos < $0 } ?? true)
                        } ?? false
                        HStack(alignment: .firstTextBaseline, spacing: 16) {
                            Text(line.time.map(stamp) ?? "--:--.--")
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(line.time != nil ? colors.accent : colors.ink3)
                                .frame(width: 64, alignment: .leading)
                            if words && !line.words.isEmpty {
                                Text(wordText(line, next: isNext ? next.word : nil)).font(.system(size: 15, weight: now ? .semibold : .regular))
                            } else {
                                Text(line.text.isEmpty ? "♪" : line.text)
                                    .font(.system(size: 15, weight: now ? .semibold : .regular))
                                    .foregroundStyle(line.time != nil ? colors.ink : colors.ink.opacity(0.45))
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 12).padding(.vertical, 7)
                        .background(selected == i ? colors.raised : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(isNext ? colors.accent.opacity(0.6) : .clear))
                        .contentShape(Rectangle())
                        .id(i)
                        .onTapGesture { choose(i, at: line.time) }
                    }
                }
                .padding(6)
            }
            .onChange(of: next.line) { _, line in withAnimation(store.motion ? .default : nil) { reader.scrollTo(line, anchor: .center) } }
        }
        .background(colors.raised.opacity(0.4), in: RoundedRectangle(cornerRadius: 10))
    }

    /// Word mode: tapped words full strength, the rest faint, the next one underlined.
    private func wordText(_ line: TimingLine, next: Int?) -> AttributedString {
        let colors = store.colors
        var text = AttributedString()
        for (w, word) in line.words.enumerated() {
            var part = AttributedString(word.text)
            if w == next {
                part.foregroundColor = colors.accent
                part.underlineStyle = .single
            } else {
                part.foregroundColor = word.time != nil ? colors.ink : colors.ink.opacity(0.4)
            }
            text += part
        }
        return text
    }

    private var tapButton: some View {
        let colors = store.colors
        let done = next.line >= lines.count
        return Button { tap() } label: {
            VStack(spacing: 2) {
                Text(done ? "All stamped. Save, or click a line to redo it." : words ? "Tap: next word" : "Tap: next line")
                    .font(.system(size: 15, weight: .semibold))
                Text("Space").font(.system(size: 11.5)).opacity(0.8)
            }
            .foregroundStyle(playingThis ? colors.accentInk : colors.ink3)
            .frame(maxWidth: .infinity, minHeight: 64)
            .background(playingThis ? colors.accent : colors.raised, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    /// −0.2 s, Redo line, +0.2 s for the line last tapped; Undo for the last tap.
    private var fixes: some View {
        HStack(spacing: 8) {
            fix("− 0.2 s", help: "The line starts earlier · ←") { timer?.earlier() }
            fix("↺ Line", help: "Tap this line again") {
                timer?.redoLine()
                if let i = selected, i < lines.count, let time = lines[i].time, playingThis { store.seek(max(0, time - 2)) }
            }
            fix("+ 0.2 s", help: "The line starts later · →") { timer?.later() }
            Button { if timer?.undo() == true { changed() } } label: {
                Label("Undo", systemImage: "arrow.uturn.backward").frame(maxWidth: .infinity)
            }
            .glassButton()
            .help("Take back the last tap · Backspace")
        }
    }

    private func fix(_ title: String, help: String, _ action: @escaping () -> Void) -> some View {
        Button { action(); changed() } label: { Text(title).frame(maxWidth: .infinity) }
            .glassButton()
            .disabled(selected == nil)
            .help(help)
    }

    private func stamp(_ t: Double) -> String {
        String(format: "%02d:%05.2f", Int(t) / 60, t.truncatingRemainder(dividingBy: 60))
    }

    /// A song's lyrics open at once (timed ones keep their times, to correct); without any,
    /// they are written here first.
    private func open() {
        guard let current = store.current?.id, current != song || timer == nil else { return }
        guard let lyrics = store.lyrics, store.lyricsFor == current,
              !lyrics.lines.isEmpty || !lyrics.plain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            if current != song { timer = nil; song = current; lines = []; dirty = false }
            return
        }
        begin(LyricTimer(lyrics: lyrics, text: ""), dirty: false)
    }

    private func begin(_ new: LyricTimer, dirty: Bool = true) {
        timer = new
        song = store.current?.id
        self.dirty = dirty
        refresh()
        tapping = true
    }

    private func refresh() {
        guard let timer else { return }
        lines = timer.lines()
        next = (Int(timer.nextLine()), Int(timer.nextWord()))
        selected = timer.selected().map(Int.init)
        words = timer.wordMode()
    }

    private func changed() {
        dirty = true
        refresh()
    }

    private func tap() {
        guard let timer, playingThis, next.line < lines.count else { return }
        timer.tap(position: store.playback?.position ?? 0)
        changed()
    }

    /// Tapping starts again from this line; a stamped line plays from a little before.
    private func choose(_ i: Int, at time: Double?) {
        timer?.select(index: UInt32(i))
        if let time, playingThis { store.seek(max(0, time - 2)) }
        refresh()
        tapping = true
    }

    private func save() {
        guard let timer, let id = song else { return }
        let result = timer.finished()
        guard !result.isEmpty else { store.fail("Tap at least one line before saving."); return }
        Task {
            if let name = await store.ask({ try $0.saveLyricsFile(id: id, lines: result) }) {
                store.notify("Saved as \(name), beside the song.")
                dirty = false
                store.reloadLyrics()
            }
        }
    }
}

/// Lyrics under the player (Settings › Appearance › Lyrics under the player): the line being
/// sung, one line, fading in at each new one; a faint ♪ while none is (the intro, a gap). Only
/// for timed lyrics, and not in the big or full screen player, which have their own.
struct LyricStrip: View {
    private var store: Store { .shared }

    @MainActor static var shows: Bool {
        let store = Store.shared
        guard store.settings.playerLyrics, store.current != nil, !store.big, !store.immersive,
              let lyrics = store.lyrics, !lyrics.instrumental else { return false }
        return !lyrics.lines.isEmpty
    }

    var body: some View {
        let colors = store.colors
        let lines = store.lyrics?.lines ?? []
        let text = store.lyricLine.flatMap { $0 < lines.count ? lines[$0].text.trimmingCharacters(in: .whitespaces) : nil } ?? ""
        ZStack {
            if text.isEmpty {
                Text("♪").foregroundStyle(colors.ink3.opacity(0.7))
                    .accessibilityLabel("No words sung now")
            } else {
                Text(text).foregroundStyle(colors.ink)
            }
        }
        .id(store.lyricLine ?? -1)
        .transition(.opacity)
        .font(.system(size: 13, weight: .medium))
        .lineLimit(1)
        .truncationMode(.tail)
        .padding(.horizontal, 14)
        .frame(height: 22)
        .frame(maxWidth: 640)
        .pane(Capsule())
        .animation(store.motion ? .easeOut(duration: 0.35) : nil, value: store.lyricLine)
        .onTapGesture { store.panel = .lyrics; store.set(["show_inspector": true]) }
        .help("Lyrics: click to see them all")
    }
}
