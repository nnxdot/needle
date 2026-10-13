import SwiftUI

/// The playing song's lyrics. Timed ones follow the song, the line sung lit, word by word when
/// the words are timed; click a line to go to it.
struct LyricsView: View {
    @Environment(Store.self) private var store
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
            if store.playback?.hasSong != true {
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
                Button("Write and time them") { store.big = false; store.navigate(.timing) }
            }
            .controlSize(.small)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func timed(_ lyrics: Lyrics, ink: Color) -> some View {
        let position = (store.playback?.position ?? 0) + 0.15
        let line = lyrics.lines.lastIndex { $0.time <= position }
        let center = kind == .big || kind == .immersive
        return ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: center ? .center : .leading, spacing: kind == .side || kind == .mini ? 12 : 18) {
                    Color.clear.frame(height: kind == .mini ? 20 : 60)
                    ForEach(Array(lyrics.lines.enumerated()), id: \.offset) { index, l in
                        lineView(l, index: index, current: line, next: index + 1 < lyrics.lines.count ? lyrics.lines[index + 1].time : nil,
                                 position: position, ink: ink, center: center)
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
    private func lineView(_ l: LyricLine, index: Int, current: Int?, next: Double?, position: Double, ink: Color, center: Bool) -> some View {
        let sung = index == current
        let past = current.map { index < $0 } ?? false
        let font = Font.system(size: size, weight: .bold)
        Group {
            if sung, !l.words.isEmpty {
                // Karaoke: the words sung so far lit, the one being sung filling.
                Text(karaoke(l, next: next, position: position, ink: ink))
            } else {
                Text(l.text.isEmpty ? "♪" : l.text)
                    .foregroundStyle(ink.opacity(sung ? 1 : past ? 0.42 : 0.3))
            }
        }
        .font(font)
        .multilineTextAlignment(center ? .center : .leading)
        .scaleEffect(sung && store.motion ? 1.02 : 1, anchor: center ? .center : .leading)
        .blur(radius: kind == .immersive && !sung && store.motion ? 0.6 : 0)
        .animation(store.motion ? .smooth(duration: 0.3) : nil, value: sung)
        .contentShape(Rectangle())
    }

    /// The line with the words sung so far lit, and the one being sung filling.
    private func karaoke(_ l: LyricLine, next: Double?, position: Double, ink: Color) -> AttributedString {
        var text = AttributedString()
        for (i, w) in l.words.enumerated() {
            let end = i + 1 < l.words.count ? l.words[i + 1].time : (next ?? w.time + 0.6)
            let lit = position >= end ? 1 : position <= w.time ? 0 : (position - w.time) / max(0.05, end - w.time)
            var word = AttributedString(w.text)
            word.foregroundColor = ink.opacity(0.35 + 0.65 * lit)
            text += word
        }
        return text
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

/// The lyric timing editor: write or paste the words, then tap Space as each line (or word)
/// starts while the song plays. Saved as a .lrc file beside the song.
struct TimingPage: View {
    @Environment(Store.self) private var store
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
        .task(id: store.current?.id) { fill() }
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

    private func fill() {
        timing = false
        stamps = []
        guard let lyrics = store.lyrics else { text = ""; return }
        text = lyrics.lines.isEmpty ? lyrics.plain : lyrics.lines.map(\.text).joined(separator: "\n")
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
        for (i, line) in lines.enumerated() {
            let mine = u.enumerated().filter { $0.element.line == i && $0.offset < stamps.count }
            guard let first = mine.first else { continue }
            let wordTimes = words ? mine.map { Word(time: stamps[$0.offset], text: $0.element.text + " ") } : []
            result.append(LyricLine(time: stamps[first.offset], text: line, words: wordTimes))
        }
        guard !result.isEmpty else { store.fail("Tap at least one line before saving."); return }
        Task { [result] in
            if let name = await store.ask({ try $0.saveLyricsFile(id: id, lines: result) }) {
                store.notify("Saved as \(name), beside the song.")
                timing = false
                store.reloadLyrics()
            }
        }
    }
}
