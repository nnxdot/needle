import SwiftUI

/// The playing song's lyrics. Timed ones follow the song, the line sung lit, word by word when
/// the words are timed; click a line to go to it.
struct LyricsView: View {
    private var store: Store { .shared }
    enum Kind { case side, big, mini, immersive }
    let kind: Kind
    var light = false

    private var size: CGFloat {
        switch kind {
        case .side: 17
        case .big: 26
        case .mini: 15
        case .immersive: 34
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
    }

    private func note(_ text: String) -> some View {
        Text(text).foregroundStyle(light ? .white.opacity(0.7) : .secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var missing: some View {
        VStack(spacing: 10) {
            Text("No lyrics for this song").font(.headline).foregroundStyle(light ? .white : store.colors.ink)
            if !store.settings.onlineMedia {
                Text("Turn on Look things up online in Settings › Library to find lyrics on LRCLIB.")
                    .font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center)
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
        let center = kind == .big || kind == .immersive
        return ScrollViewReader { reader in
            ScrollView {
                LazyVStack(alignment: center ? .center : .leading, spacing: kind == .side || kind == .mini ? 12 : 18) {
                    Color.clear.frame(height: kind == .mini ? 20 : 60)
                    ForEach(Array(lyrics.lines.enumerated()), id: \.offset) { index, l in
                        let state: LyricLineView.State = index == line ? .sung : (line.map { index < $0 } ?? false) ? .past : .ahead
                        Group {
                            if state == .sung, !l.words.isEmpty {
                                SungLine(line: l, next: index + 1 < lyrics.lines.count ? lyrics.lines[index + 1].time : nil,
                                         size: size, ink: ink, center: center)
                            } else {
                                LyricLineView(text: l.text, state: state, size: size, ink: ink, center: center, motion: store.motion)
                                    .equatable()
                            }
                        }
                        .id(index)
                        .onTapGesture { store.seek(l.time) }
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

    @ViewBuilder
    private func footer(_ lyrics: Lyrics) -> some View {
        if kind == .side || kind == .big {
            HStack(spacing: 10) {
                Text(source(lyrics.source)).font(.caption).foregroundStyle(light ? .white.opacity(0.6) : .secondary)
                Button("Time these lyrics") { store.big = false; store.navigate(.timing) }
                    .buttonStyle(.link).font(.caption)
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

/// The lyric timing editor: write or paste the words, then tap Space as each line (or word)
/// starts while the song plays. Saved as a .lrc file beside the song.
struct TimingPage: View {
    private var store: Store { .shared }
    @State private var text = ""
    @State private var words = false
    @State private var timing = false
    /// The time stamped for each line (or word), in order.
    @State private var stamps: [Double] = []
    @FocusState private var tapping: Bool

    private var lines: [String] {
        text.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    /// What one tap stamps: lines, or words of every line.
    private var units: [(line: Int, text: String)] {
        words
            ? lines.enumerated().flatMap { i, l in l.split(separator: " ").map { (i, String($0)) } }
            : lines.enumerated().map { ($0.offset, $0.element) }
    }

    var body: some View {
        let colors = store.colors
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("Lyric timing").font(store.display(28))
                Spacer()
                Picker("", selection: $words) {
                    Text("Lines").tag(false)
                    Text("Words").tag(true)
                }
                .pickerStyle(.segmented)
                .frame(width: 160)
                .disabled(timing)
            }
            if store.current == nil {
                Text("Play a song first, then time its lyrics.").foregroundStyle(.secondary)
            } else if store.current?.cue != nil {
                Text("This song is part of a CUE sheet, so it has no file of its own to put lyrics beside.").foregroundStyle(.secondary)
            } else if !timing {
                Text(lines.isEmpty ? "This song has no lyrics yet. Write them here, then tap along to time them." : "Paste or type the words here, one line per sung line.")
                    .foregroundStyle(colors.ink2)
                TextEditor(text: $text)
                    .font(.system(size: 14))
                    .scrollContentBackground(.hidden)
                    .padding(8)
                    .background(colors.raised, in: RoundedRectangle(cornerRadius: 10))
                HStack {
                    Button { start() } label: { Label("Start timing", systemImage: "play.fill") }
                        .glassButton(prominent: true)
                        .disabled(lines.isEmpty)
                    Text("Play this song, then tap along.").font(.caption).foregroundStyle(.secondary)
                }
            } else {
                Text(words ? "Press Space as each word starts. Backspace undoes. ← and → move the last one by a twentieth of a second."
                           : "Press Space as each line starts. Backspace undoes. ← and → move the last line by a twentieth of a second.")
                    .foregroundStyle(colors.ink2)
                ScrollViewReader { reader in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                                HStack(alignment: .firstTextBaseline, spacing: 14) {
                                    Text(stampFor(line: i).map(clockStamp) ?? "—")
                                        .font(.system(size: 12, design: .monospaced))
                                        .foregroundStyle(colors.ink3)
                                        .frame(width: 70, alignment: .trailing)
                                    Text(line)
                                        .font(.system(size: 16, weight: i == currentLine ? .bold : .regular))
                                        .foregroundStyle(i == currentLine ? colors.accent : stampFor(line: i) != nil ? colors.ink : colors.ink3)
                                }
                                .id(i)
                                .onTapGesture { redo(from: i) }
                            }
                        }
                        .padding(.vertical, 10)
                    }
                    .onChange(of: stamps.count) { _, _ in withAnimation { reader.scrollTo(currentLine, anchor: .center) } }
                }
                .background(colors.raised.opacity(0.6), in: RoundedRectangle(cornerRadius: 10))
                HStack {
                    Button { tap() } label: { Label(words ? "Tap: next word" : "Tap: next line", systemImage: "hand.tap") }
                        .glassButton(prominent: true)
                        .keyboardShortcut(.space, modifiers: [])
                        .disabled(stamps.count >= units.count)
                    Button("Save") { save() }.glassButton().disabled(stamps.isEmpty)
                    Button("Close") { timing = false; stamps = [] }.glassButton()
                    Spacer()
                    if stamps.count >= units.count {
                        Text("All stamped. Save, or click a line to redo it.").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(28)
        .frame(maxWidth: 820, maxHeight: .infinity, alignment: .topLeading)
        .frame(maxWidth: .infinity)
        .focusable(timing)
        .focused($tapping)
        .onKeyPress(.delete) { guard timing, !stamps.isEmpty else { return .ignored }; stamps.removeLast(); return .handled }
        .onKeyPress(.leftArrow) { nudge(-0.05) }
        .onKeyPress(.rightArrow) { nudge(0.05) }
        .task(id: "\(store.current?.id ?? "")|\(store.lyricsFor ?? "")") { fill() }
    }

    private var currentLine: Int {
        let u = units
        return stamps.count < u.count ? u[stamps.count].line : max(0, lines.count - 1)
    }

    private func stampFor(line: Int) -> Double? {
        units.enumerated().first { $0.element.line == line && $0.offset < stamps.count }.map { stamps[$0.offset] }
    }

    private func clockStamp(_ t: Double) -> String {
        String(format: "%d:%05.2f", Int(t) / 60, t.truncatingRemainder(dividingBy: 60))
    }

    /// What this page last filled in, to tell it from words typed here.
    @State private var filled = ""
    @State private var filledSong: String?

    private func fill() {
        let song = store.current?.id
        if song != filledSong {
            // Another song: its own words, and no timing carried over from the last one.
            filledSong = song
            timing = false
            stamps = []
        } else if text != filled {
            // The same song's lyrics came, but words were typed here: they stay.
            return
        }
        let found = store.lyrics.map { $0.lines.isEmpty ? $0.plain : $0.lines.map(\.text).joined(separator: "\n") } ?? ""
        text = found
        filled = found
    }

    private func start() {
        stamps = []
        timing = true
        tapping = true
        store.seek(0)
        if store.playback?.playing != true { store.toggle() }
    }

    private func tap() {
        guard stamps.count < units.count else { return }
        stamps.append(store.playback?.position ?? 0)
    }

    private func nudge(_ by: Double) -> KeyPress.Result {
        guard timing, !stamps.isEmpty else { return .ignored }
        stamps[stamps.count - 1] = max(0, stamps[stamps.count - 1] + by)
        return .handled
    }

    private func redo(from line: Int) {
        guard timing, let first = units.firstIndex(where: { $0.line == line }), first < stamps.count else { return }
        stamps.removeSubrange(first...)
        if let t = first > 0 ? stamps.last : 0 { store.seek(max(0, t - 2)) }
    }

    private func save() {
        guard let id = store.current?.id else { return }
        let u = units
        var result: [LyricLine] = []
        // The song's own times, line for line, for the lines not tapped this time.
        // (Lines with no words, such as instrumental marks, are not lines here.)
        let before = store.lyrics.flatMap { lyrics -> [LyricLine]? in
            let worded = lyrics.lines.filter { !$0.text.trimmingCharacters(in: .whitespaces).isEmpty }
            return worded.count == lines.count ? worded : nil
        }
        for (i, line) in lines.enumerated() {
            let mine = u.enumerated().filter { $0.element.line == i && $0.offset < stamps.count }
            guard let first = mine.first else {
                guard let old = before?[i], old.text.trimmingCharacters(in: .whitespaces) == line else {
                    store.fail("Tap every line before saving: lines without a time would be lost from the file.")
                    return
                }
                result.append(old)
                continue
            }
            let wordTimes = words ? mine.map { Word(time: stamps[$0.offset], text: $0.element.text + " ") } : []
            result.append(LyricLine(time: stamps[first.offset], text: line, words: wordTimes))
        }
        guard !result.isEmpty else { store.fail("Tap at least one line before saving."); return }
        result.sort { $0.time < $1.time }
        Task { [result] in
            if let name = await store.ask({ try $0.saveLyricsFile(id: id, lines: result) }) {
                store.notify("Saved as \(name), beside the song.")
                timing = false
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
