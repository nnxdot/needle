import SwiftUI

/// The player at the foot of the window, floating on glass: the song, the controls and seek
/// bar, and the output, volume, and panels.
struct PlayerBar: View {
    private var store: Store { .shared }

    enum Width { case wide, medium, narrow }

    var body: some View {
        GeometryReader { geo in
            let width: Width = geo.size.width > 900 ? .wide : geo.size.width > 620 ? .medium : .narrow
            bar(width)
                .padding(.horizontal, width == .narrow ? 12 : 18)
                .frame(width: geo.size.width, height: geo.size.height)
        }
        .frame(height: 72)
        .pane(Capsule())
    }

    /// The bar: the song on the left, the controls and seek bar in the middle, the output and
    /// panels on the right. Narrower, the song gives up its words and the extras go in a menu.
    private func bar(_ width: Width) -> some View {
        HStack(spacing: width == .narrow ? 10 : 18) {
            // A set width: given only what was left, the song's words shrank to nothing.
            NowPlayingInfo(coverSide: width == .narrow ? 40 : 48, heart: width != .narrow)
                .frame(width: width == .wide ? 280 : width == .medium ? 210 : 150, alignment: .leading)
            VStack(spacing: 3) {
                Transport(play: 34, compact: width == .narrow)
                SeekBar(small: width == .narrow)
            }
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
            VStack(alignment: .trailing, spacing: 3) {
                if width == .wide { SignalPath() }
                HStack(spacing: width == .narrow ? 8 : 12) {
                    if width != .narrow { VolumeControl(width: width == .wide ? 96 : 64) }
                    if width == .wide {
                        SoundButton()
                        SpeakersButton()
                    }
                    if width != .narrow { PanelButton(panel: .lyrics, icon: "quote.bubble", help: "Lyrics · ⌘L") }
                    QueueButton()
                    MoreButton(all: width != .wide)
                }
                .font(.system(size: 14))
            }
            .layoutPriority(1)
        }
    }
}

/// The player's other buttons, in a menu: the A–B loop and the mini player, and (narrow) the
/// sound tools.
struct MoreButton: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    let all: Bool

    var body: some View {
        let looping = store.playback?.loopStart != nil || store.loopStart != nil
        Menu {
            Button(looping ? "Clear the A–B Loop" : store.loopStart != nil ? "Set Point B" : "A–B Loop: Set Point A") { store.loopButton() }
                .disabled(store.playback?.hasSong != true)
            Button("Mini Player") { openWindow(id: "mini") }
            if all {
                Divider()
                Button("Equalizer and Sound Tools") { store.navigate(.sound) }
            }
        } label: {
            Image(systemName: "ellipsis.circle").foregroundStyle(looping ? store.colors.accent : store.colors.ink2)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .help("More")
    }
}

/// The player in the title bar, as in Apple Music: the controls, the song and its time, and
/// the volume and panels.
struct TopPlayer: View {
    private var store: Store { .shared }

    var body: some View {
        HStack(spacing: 14) {
            Transport(play: 26, compact: true)
            HStack(spacing: 8) {
                if let p = store.playback, p.hasSong {
                    Button { store.big = true } label: {
                        Cover(path: p.artwork, seed: p.album + p.artist, side: 30, corner: 4)
                    }
                    .buttonStyle(.plain)
                    .help("Open the big player · ⌘P")
                }
                VStack(spacing: 1) {
                    if let p = store.playback, p.hasSong {
                        Button("\(p.title) · \(p.artist)") { if let c = store.current { store.navigate(c.albumRoute) } }
                            .buttonStyle(.plain)
                            .font(.callout)
                            .lineLimit(1)
                    } else {
                        Text("Nothing playing").font(.callout).foregroundStyle(.secondary)
                    }
                    SeekBar(small: true)
                }
                .frame(minWidth: 220, idealWidth: 320, maxWidth: 360)
                if let c = store.current {
                    Heart(on: c.favorite) { store.toggleFavorite([c]) }
                }
            }
            .padding(.horizontal, 8)
            HStack(spacing: 10) {
                VolumeControl(width: 80)
                PanelButton(panel: .lyrics, icon: "quote.bubble", help: "Lyrics · ⌘L")
                QueueButton()
            }
        }
        .contextMenu { if let c = store.current { SongMenu(tracks: [c], playing: true) } }
    }
}

/// The cover, title, and artist of the song playing, and its heart. The cover opens the big
/// player; right-click for the song's menu.
struct NowPlayingInfo: View {
    private var store: Store { .shared }
    let coverSide: CGFloat
    var heart = true

    var body: some View {
        let colors = store.colors
        HStack(spacing: 12) {
            if let p = store.playback, p.hasSong {
                Button { store.big = true } label: {
                    Cover(path: p.artwork, seed: p.album + p.artist, side: coverSide, corner: 6)
                        .shadow(color: .black.opacity(0.2), radius: 4, y: 2)
                }
                .buttonStyle(.plain)
                .help("Open the big player · ⌘P")
                VStack(alignment: .leading, spacing: 2) {
                    Button(p.title) { if let c = store.current { store.navigate(c.albumRoute) } }
                        .buttonStyle(.plain)
                        .font(.body.weight(.medium))
                        .lineLimit(1)
                    Button(p.artist) { store.navigate(.artist(store.current?.artist ?? p.artist)) }
                        .buttonStyle(.plain)
                        .font(.callout)
                        .foregroundStyle(colors.ink2)
                        .lineLimit(1)
                }
                if heart, let c = store.current {
                    Heart(on: c.favorite, size: 14) { store.toggleFavorite([c]) }
                }
            } else {
                RoundedRectangle(cornerRadius: 6).fill(colors.raised).frame(width: coverSide, height: coverSide)
                    .overlay { Image(systemName: "music.note").foregroundStyle(colors.ink3) }
                VStack(alignment: .leading, spacing: 2) {
                    Text("Nothing playing").foregroundStyle(colors.ink2)
                    Text("Double-click a track to start.").font(.caption).foregroundStyle(colors.ink3)
                }
            }
        }
        .contextMenu { if let c = store.current { SongMenu(tracks: [c], playing: true) } }
    }
}

/// Shuffle, previous, play or pause, next, and repeat.
struct Transport: View {
    private var store: Store { .shared }
    let play: CGFloat
    var compact = false
    var light = false

    var body: some View {
        let colors = store.colors
        let playing = store.playback?.playing == true
        let repeatMode = store.playback?.repeat ?? "off"
        HStack(spacing: compact ? 4 : 10) {
            if !compact {
                Button { store.shuffleView() } label: { Image(systemName: "shuffle") }
                    .help(store.route.isSongs ? "Shuffle this view" : "Shuffle up next")
            }
            Button { store.previous() } label: { Image(systemName: "backward.fill") }
                .help("Previous · ⌘←")
            Button { store.toggle() } label: {
                Image(systemName: playing ? "pause.fill" : "play.fill")
                    .font(.system(size: play * 0.42, weight: .bold))
                    .frame(width: play, height: play)
                    .foregroundStyle(light ? .black : colors.canvas)
                    .background(light ? .white : colors.ink, in: Circle())
                    .contentTransition(.symbolEffect(.replace))
            }
            .help(playing ? "Pause · Space" : "Play · Space")
            Button { store.next() } label: { Image(systemName: "forward.fill") }
                .help("Next · ⌘→")
            if !compact {
                Button { store.cycleRepeat() } label: {
                    Image(systemName: repeatMode == "one" ? "repeat.1" : "repeat")
                        .foregroundStyle(repeatMode == "off" ? (light ? .white.opacity(0.8) : colors.ink2) : colors.accent)
                }
                .help(repeatMode == "off" ? "Repeat is off" : repeatMode == "all" ? "Repeating the queue" : "Repeating this track")
            }
        }
        .buttonStyle(.borderless)
        .font(.system(size: compact ? 13 : 15))
        .foregroundStyle(light ? .white : colors.ink)
    }
}

/// The seek bar. While it is held the music is silent and the bar only moves; letting go
/// jumps once to where it was left and plays on.
struct SeekBar: View {
    private var store: Store { .shared }
    var small = false
    var light = false
    @State private var held: Double?

    var body: some View {
        let p = store.playback
        let duration = max(p?.duration ?? 0, 0.01)
        let position = held ?? p?.position ?? 0
        HStack(spacing: 8) {
            if !small {
                Text(clock(p?.hasSong == true ? position : 0)).monospacedDigit().frame(width: 42, alignment: .trailing)
            }
            Slider(value: Binding(get: { min(position, duration) }, set: { held = $0 }), in: 0...duration, onEditingChanged: { editing in
                if editing {
                    store.core.scrub(held: true)
                } else {
                    if let to = held { store.seek(to) }
                    store.core.scrub(held: false)
                    Task { try? await Task.sleep(for: .milliseconds(500)); held = nil }
                }
            })
            .controlSize(small ? .mini : .small)
            .disabled(p?.hasSong != true)
            .overlay { LoopMarks(duration: duration) }
            if !small {
                Text(clock(p?.duration ?? 0)).monospacedDigit().frame(width: 42, alignment: .leading)
            }
        }
        .font(.caption)
        .foregroundStyle(light ? .white.opacity(0.8) : store.colors.ink3)
    }
}

/// The A–B loop's stretch, over the seek bar.
private struct LoopMarks: View {
    private var store: Store { .shared }
    let duration: Double

    var body: some View {
        GeometryReader { geo in
            if let p = store.playback, let a = p.loopStart, let b = p.loopEnd {
                let x = geo.size.width * a / duration, w = geo.size.width * (b - a) / duration
                Capsule().fill(store.colors.accent.opacity(0.35)).frame(width: max(2, w), height: 4)
                    .position(x: x + w / 2, y: geo.size.height / 2)
            } else if let a = store.loopStart {
                Capsule().fill(store.colors.accent).frame(width: 2, height: 10)
                    .position(x: geo.size.width * a / duration, y: geo.size.height / 2)
            }
        }
        .allowsHitTesting(false)
    }
}

/// What plays and how: "FLAC 24/96 → 48 kHz"; it opens Settings › Playback.
struct SignalPath: View {
    private var store: Store { .shared }
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        if let p = store.playback, p.hasSong, let c = store.current {
            let (short, detail) = path(p, c)
            Button { openSettings() } label: {
                Label(short, systemImage: "waveform.path")
                    .font(.caption)
                    .lineLimit(1)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 2)
                    .background(store.colors.raised.opacity(0.8), in: Capsule())
            }
            .buttonStyle(.plain)
            .foregroundStyle(store.colors.ink2)
            .help(detail)
        }
    }

    private func path(_ p: MacPlayback, _ c: Track) -> (String, String) {
        let source = c.quality
        guard p.outputRate > 0 else { return (source, "\(source) · the output opens when playback starts") }
        let k = Double(p.outputRate) / 1000
        let rate = k.rounded() == k ? "\(Int(k)) kHz" : String(format: "%.1f kHz", k)
        let resampled = c.sampleRate != Int(p.outputRate)
        let gain = p.replayGain ? "ReplayGain and volume" : "volume"
        return ("\(source) → \(rate)", "\(source) →\(resampled ? " resampled →" : "") \(gain) → Core Audio at \(rate) → \(p.output)")
    }
}

struct LoopButton: View {
    private var store: Store { .shared }

    var body: some View {
        let looping = store.playback?.loopStart != nil
        Button { store.loopButton() } label: {
            Image(systemName: "repeat.circle")
                .foregroundStyle(looping || store.loopStart != nil ? store.colors.accent : store.colors.ink2)
        }
        .buttonStyle(.borderless)
        .disabled(store.playback?.hasSong != true)
        .help(looping ? "A–B loop on · click to clear" : store.loopStart != nil ? "Point A set · click again to set B and start looping" : "A–B loop · click to set point A")
    }
}

/// Mute and the volume slider.
struct VolumeControl: View {
    private var store: Store { .shared }
    let width: CGFloat
    var light = false

    var body: some View {
        let volume = store.playback?.volume ?? 0.65
        HStack(spacing: 4) {
            Button { store.toggleMute() } label: {
                Image(systemName: volume <= 0.001 ? "speaker.slash.fill" : volume < 0.5 ? "speaker.wave.1.fill" : "speaker.wave.3.fill")
                    .frame(width: 18)
            }
            .buttonStyle(.borderless)
            .help("Mute")
            Slider(value: Binding(get: { Double(volume) }, set: { store.setVolume(Float($0)) }), in: 0...1)
                .controlSize(.mini)
                .frame(width: width)
        }
        .foregroundStyle(light ? .white : store.colors.ink2)
        // The wheel over the volume changes it.
        .background {
            GeometryReader { geo in
                Color.clear
                    .onAppear { VolumeWheel.areas[id] = geo.frame(in: .global) }
                    .onChange(of: geo.frame(in: .global)) { _, frame in VolumeWheel.areas[id] = frame }
                    .onDisappear { VolumeWheel.areas[id] = nil }
            }
        }
    }

    @State private var id = UUID()
}

/// Where the volume controls are, so the scroll wheel over them changes the volume
/// (see `Keys`).
@MainActor
enum VolumeWheel {
    static var areas: [UUID: CGRect] = [:]
    /// Part of a step from a touchpad, kept until it makes a whole one.
    static var partial: CGFloat = 0

    /// Whether `event` is over a volume control; it then changes the volume.
    static func handle(_ event: NSEvent, store: Store) -> Bool {
        guard let window = event.window, let content = window.contentView else { return false }
        let p = event.locationInWindow
        // SwiftUI's global frames start at the top left of the window's content.
        let point = CGPoint(x: p.x, y: content.bounds.height - p.y)
        guard areas.values.contains(where: { $0.insetBy(dx: -4, dy: -6).contains(point) }) else { return false }
        var steps: CGFloat
        if event.hasPreciseScrollingDeltas {
            partial += event.scrollingDeltaY / 40
            steps = partial.rounded(.towardZero)
            partial -= steps
        } else {
            steps = event.scrollingDeltaY > 0 ? 1 : event.scrollingDeltaY < 0 ? -1 : 0
        }
        if event.isDirectionInvertedFromDevice { steps = -steps }
        if steps != 0 { store.nudgeVolume(Float(steps) * 0.05) }
        return true
    }
}

/// Opens the equalizer and sound tools (lit while any is on).
struct SoundButton: View {
    private var store: Store { .shared }
    @State private var on = false

    var body: some View {
        Button { store.navigate(.sound) } label: {
            Image(systemName: "slider.vertical.3").foregroundStyle(on ? store.colors.accent : store.colors.ink2)
        }
        .buttonStyle(.borderless)
        .help("Equalizer and sound tools")
        .task(id: store.libraryVersion) {
            if let s = try? store.core.soundSettings() {
                on = s.eq || s.mono || s.crossfeed || abs(s.balance) > 0.001
            }
        }
    }
}

/// Opens and closes a tab of the side panel.
struct PanelButton: View {
    private var store: Store { .shared }
    let panel: Store.Panel
    let icon: String
    let help: String

    var body: some View {
        let open = store.settings.showInspector && store.panel == panel
        Button {
            if open { store.set(["show_inspector": false]) } else { store.panel = panel; store.set(["show_inspector": true]) }
        } label: {
            Image(systemName: icon).foregroundStyle(open ? store.colors.accent : store.colors.ink2)
        }
        .buttonStyle(.borderless)
        .help(help)
    }
}

/// The queue's button; songs dropped on it join the queue.
struct QueueButton: View {
    private var store: Store { .shared }

    var body: some View {
        PanelButton(panel: .queue, icon: "list.bullet", help: "Queue · ⌘J")
            .dropDestination(for: String.self) { ids, _ in
                store.enqueue(ids)
                return true
            }
    }
}

struct MiniButton: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button { openWindow(id: "mini") } label: { Image(systemName: "pip").foregroundStyle(store.colors.ink2) }
            .buttonStyle(.borderless)
            .help("Mini player · ⌥⌘M")
    }
}

/// Play on this Mac or on speakers on the network (Chromecast, AirPlay, DLNA), one or several.
struct SpeakersButton: View {
    private var store: Store { .shared }
    @State private var open = false

    var body: some View {
        let speaker = store.settings.outputDevice?.hasPrefix("speaker") == true
        Button { open.toggle() } label: {
            Image(systemName: "airplayaudio").foregroundStyle(speaker ? store.colors.accent : store.colors.ink2)
        }
        .buttonStyle(.borderless)
        .help(speaker ? "Playing on another speaker" : "Play on another speaker")
        .popover(isPresented: $open, arrowEdge: .top) { SpeakersPicker().environment(store) }
    }
}

struct SpeakersPicker: View {
    private var store: Store { .shared }
    @Environment(\.dismiss) private var dismiss
    @State private var found: [Speaker]?
    @State private var picking = false
    @State private var chosen: Set<Speaker> = []
    @State private var thisMac = true

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(picking ? "Play together on" : "Play on").font(.headline)
            if picking {
                Toggle("This Mac", isOn: $thisMac)
                ForEach(found ?? [], id: \.id) { s in
                    Toggle("\(s.name) · \(s.label)", isOn: Binding(get: { chosen.contains(s) }, set: { if $0 { chosen.insert(s) } else { chosen.remove(s) } }))
                }
                if found == nil { ProgressView("Looking for speakers…").controlSize(.small) }
                HStack {
                    Button("Back") { picking = false }
                    Spacer()
                    Button("Play on These \(chosen.count + (thisMac ? 1 : 0))") { playOn(Array(chosen), thisMac) }
                        .disabled(chosen.count + (thisMac ? 1 : 0) < 2)
                        .buttonStyle(.borderedProminent)
                }
            } else {
                row("This Mac", on: store.settings.outputDevice.map { !$0.hasPrefix("speaker") } ?? true) { playOn([], true) }
                ForEach(found ?? [], id: \.id) { s in
                    row("\(s.name) · \(s.label)", on: store.settings.outputDevice?.contains(s.address) == true) { playOn([s], false) }
                }
                if found == nil {
                    ProgressView("Looking for speakers…").controlSize(.small)
                } else if found?.isEmpty == true {
                    Text("No speakers answered. Needle looks for Chromecast, AirPlay, and DLNA speakers on this network.")
                        .font(.caption).foregroundStyle(.secondary).frame(width: 260, alignment: .leading)
                }
                Divider()
                Button("Play on Several at Once…") { picking = true }.buttonStyle(.borderless)
            }
        }
        .padding(14)
        .frame(minWidth: 280, alignment: .leading)
        .task {
            let json = await Task.detached { [core = store.core] in core.findSpeakersJson() }.value
            found = JSON.decode([Speaker].self, json, plain: true) ?? []
        }
    }

    private func row(_ name: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Image(systemName: "hifispeaker").frame(width: 18)
                Text(name)
                Spacer()
                if on { Image(systemName: "checkmark").foregroundStyle(store.colors.accent) }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.vertical, 3)
    }

    private func playOn(_ speakers: [Speaker], _ thisMac: Bool) {
        let json: String? = speakers.isEmpty ? nil : String(decoding: (try? JSONEncoder().encode(speakers)) ?? Data(), as: UTF8.self)
        Task {
            if let name = await store.ask({ try $0.playOn(speakers: json, thisComputer: thisMac) }) {
                store.reloadSettings()
                store.notify(speakers.isEmpty ? "Playing on this Mac." : "Playing on \(name). It may take a few seconds to start.")
            }
        }
        dismiss()
    }
}
