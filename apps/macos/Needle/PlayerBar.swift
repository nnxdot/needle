import SwiftUI

/// The player, after Apple Music's: one capsule of glass floating over the page, with the
/// controls, a small screen with the song and a thin line for where it is, and the panels and
/// volume, at the foot of the page. (`TopPlayer` is the same in the title bar.)
struct PlayerBar: View {
    private var store: Store { .shared }

    var body: some View {
        GeometryReader { geo in
            let narrow = geo.size.width < 600, wide = geo.size.width > 720
            HStack(spacing: narrow ? 10 : 18) {
                Transport(play: 30, compact: narrow, bare: true)
                    .fixedSize()
                NowScreen()
                    .frame(maxWidth: .infinity)
                HStack(spacing: narrow ? 10 : 14) {
                    if !narrow, let c = store.current {
                        Heart(on: c.favorite, size: 14) { store.toggleFavorite([c]) }
                    }
                    PanelButton(panel: .lyrics, icon: "quote.bubble", help: "Lyrics · ⌘L")
                    QueueButton()
                    if wide { SpeakersButton() }
                    MoreButton(all: true)
                    VolumeControl(width: wide ? 64 : 0)
                }
                .font(.system(size: 14))
                .fixedSize()
            }
            .padding(.leading, narrow ? 16 : 22)
            .padding(.trailing, narrow ? 14 : 20)
            .frame(width: geo.size.width, height: geo.size.height)
        }
        .frame(height: 56)
        .frame(maxWidth: 820)
        .pane(Capsule())
    }
}

/// The player in the title bar: the same controls and small screen as the player bar, smaller,
/// in the bar's own glass.
struct TopPlayer: View {
    private var store: Store { .shared }

    var body: some View {
        HStack(spacing: 14) {
            Transport(play: 22, compact: true, bare: true, icons: 13)
                .fixedSize()
            NowScreen(small: true)
                .frame(minWidth: 160, idealWidth: 280, maxWidth: 340)
            HStack(spacing: 12) {
                if let c = store.current {
                    Heart(on: c.favorite, size: 13) { store.toggleFavorite([c]) }
                }
                PanelButton(panel: .lyrics, icon: "quote.bubble", help: "Lyrics · ⌘L")
                QueueButton()
                MoreButton(all: true)
                VolumeControl(width: 56)
            }
            .font(.system(size: 13))
            .fixedSize()
        }
        .padding(.horizontal, 10)
    }
}

/// The player's small screen: the cover (it opens the big player), the title (it opens the
/// album) and artist, and along its foot a thin line for where the song is; click or drag the
/// line to move. The times show while the pointer is over it.
struct NowScreen: View {
    private var store: Store { .shared }
    /// Smaller, for the title bar.
    var small = false
    @State private var held: Double?
    @State private var hover = false

    var body: some View {
        let colors = store.colors
        let side: CGFloat = small ? 22 : 34
        let p = store.playback
        let has = p?.hasSong == true
        let duration = max(p?.duration ?? 0, 0.01)
        let position = held.map { $0 * duration } ?? p?.position ?? 0
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                if let p, has {
                    Button { store.big = true } label: { Cover(path: p.artwork, seed: p.album + p.artist, side: side, corner: small ? 4 : 5) }
                        .buttonStyle(.plain)
                        .help("Open the big player · ⌘P")
                    VStack(alignment: .leading, spacing: small ? 0 : 1) {
                        HStack(spacing: 8) {
                            Button { if let c = store.current { store.navigate(c.albumRoute) } } label: {
                                Text(p.title).font(.system(size: small ? 11.5 : 13, weight: .semibold)).foregroundStyle(colors.ink).lineLimit(1)
                            }
                            .buttonStyle(.plain)
                            Spacer(minLength: 0)
                            if hover {
                                Text("\(clock(position)) / \(clock(p.duration))")
                                    .font(.system(size: small ? 9 : 10).monospacedDigit())
                                    .foregroundStyle(colors.ink3)
                                    .fixedSize()
                            }
                        }
                        Button { store.navigate(.artist(store.current?.artist ?? p.artist)) } label: {
                            Text([p.artist, p.album].filter { !$0.isEmpty }.joined(separator: " — "))
                                .font(.system(size: small ? 10.5 : 12))
                                .foregroundStyle(colors.ink2)
                                .lineLimit(1)
                        }
                        .buttonStyle(.plain)
                    }
                } else {
                    RoundedRectangle(cornerRadius: 5).fill(colors.raised).frame(width: side, height: side)
                        .overlay { Image(systemName: "music.note").foregroundStyle(colors.ink3) }
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Nothing playing").font(.system(size: small ? 12 : 13, weight: .semibold)).foregroundStyle(colors.ink2)
                        Text("Double-click a track to start.").font(.system(size: small ? 11 : 12)).foregroundStyle(colors.ink3)
                    }
                }
                Spacer(minLength: 0)
            }
            // In the title bar it takes only the height its words need: the bar is low.
            .frame(height: small ? nil : 38)
            ThinBar(value: min(1, position / duration), tint: colors.accent, area: small ? 4 : 10) { held = $0 } began: {
                store.core.scrub(held: true)
            } ended: { fraction in
                store.seek(fraction * duration)
                store.core.scrub(held: false)
                Task { try? await Task.sleep(for: .milliseconds(400)); held = nil }
            }
            .disabled(!has)
            .overlay { LoopMarks(duration: duration) }
        }
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .contextMenu { if let c = store.current { SongMenu(tracks: [c], playing: true) } }
    }
}

/// The player's other things, in a menu: what plays and how, the A–B loop, the mini player,
/// and the sound tools.
struct MoreButton: View {
    private var store: Store { .shared }
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings
    let all: Bool

    var body: some View {
        let looping = store.playback?.loopStart != nil || store.loopStart != nil
        Menu {
            if let p = store.playback, p.hasSong, let c = store.current {
                Button(SignalPath.describe(p, c).0) { openSettings() }
                Divider()
            }
            Button(looping ? "Clear the A–B Loop" : store.loopStart != nil ? "Set Point B" : "A–B Loop: Set Point A") { store.loopButton() }
                .disabled(store.playback?.hasSong != true)
            Button("Mini Player") { openWindow(id: "mini") }
            if all {
                Divider()
                Button("Equalizer and Sound Tools") { store.navigate(.sound) }
            }
        } label: {
            Image(systemName: "ellipsis").foregroundStyle(looping ? store.colors.accent : store.colors.ink2)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        // A borderless menu draws its label in the tint, not the label's colour.
        .tint(looping ? store.colors.accent : store.colors.ink2)
        .fixedSize()
        .help("More")
    }
}

/// A thin line for a value from 0 to 1 (where a song is, the volume), thicker under the
/// pointer; click or drag along it to change it.
struct ThinBar: View {
    let value: Double
    var tint: Color
    var track: Color = Color.primary.opacity(0.16)
    /// The height that takes clicks; the line is drawn in its middle.
    var area: CGFloat = 12
    let changed: (Double) -> Void
    var began: () -> Void = {}
    var ended: (Double) -> Void = { _ in }
    @State private var hover = false
    @State private var dragged: Double?
    @Environment(\.isEnabled) private var enabled

    var body: some View {
        GeometryReader { geo in
            let fraction = min(1, max(0, dragged ?? value))
            ZStack(alignment: .leading) {
                Capsule().fill(track)
                Capsule().fill(tint).frame(width: geo.size.width * fraction)
            }
            .frame(height: hover || dragged != nil ? 5 : 3)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { drag in
                    let next = min(1, max(0, drag.location.x / max(1, geo.size.width)))
                    if dragged == nil { began() }
                    dragged = next
                    changed(next)
                }
                .onEnded { _ in
                    if let last = dragged { ended(last) }
                    dragged = nil
                })
        }
        .frame(height: area)
        .opacity(enabled ? 1 : 0.5)
        .onHover { hover = $0 }
        .animation(.easeOut(duration: 0.12), value: hover)
    }
}

/// Shuffle, previous, play or pause, next, and repeat.
struct Transport: View {
    private var store: Store { .shared }
    let play: CGFloat
    var compact = false
    var light = false
    /// The play button as a plain sign, with no circle (the player bar's).
    var bare = false
    /// The size of the signs, when not the usual.
    var icons: CGFloat?

    var body: some View {
        let colors = store.colors
        let playing = store.playback?.playing == true
        let repeatMode = store.playback?.repeat ?? "off"
        HStack(spacing: bare ? (icons ?? 16) * 0.85 : compact ? 4 : 10) {
            if !compact {
                Button { store.shuffleView() } label: { Image(systemName: "shuffle") }
                    .help(store.route.isSongs ? "Shuffle this view" : "Shuffle up next")
            }
            Button { store.previous() } label: { Image(systemName: "backward.fill") }
                .help("Previous · ⌘←")
            Button { store.toggle() } label: {
                if bare {
                    Image(systemName: playing ? "pause.fill" : "play.fill")
                        .font(.system(size: play * 0.7))
                        .frame(width: play, height: play)
                        .contentShape(Rectangle())
                        .contentTransition(.symbolEffect(.replace))
                } else {
                    Image(systemName: playing ? "pause.fill" : "play.fill")
                        .font(.system(size: play * 0.42, weight: .bold))
                        .frame(width: play, height: play)
                        .foregroundStyle(light ? .black : colors.canvas)
                        .background(light ? .white : colors.ink, in: Circle())
                        .contentTransition(.symbolEffect(.replace))
                }
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
        .font(.system(size: icons ?? (bare ? 16 : compact ? 13 : 15)))
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

    private func path(_ p: MacPlayback, _ c: Track) -> (String, String) { Self.describe(p, c) }

    /// What plays and how, short ("FLAC 24/96 → 48 kHz") and in full.
    static func describe(_ p: MacPlayback, _ c: Track) -> (String, String) {
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
            if width > 0 {
                ThinBar(value: Double(volume), tint: light ? .white : store.colors.ink2,
                        track: light ? .white.opacity(0.25) : Color.primary.opacity(0.16)) { store.setVolume(Float($0)) }
                    .frame(width: width)
            }
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
