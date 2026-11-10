import AppKit
import Carbon.HIToolbox
import SwiftUI

/// Keys that work everywhere, even with Needle in the background (Settings › Keyboard ›
/// Shortcuts everywhere), as on Windows. Carbon's hot keys (`RegisterEventHotKey`): they need no
/// Accessibility permission, and Apple's sandbox allows them. Off until turned on here (a Mac
/// switch, kept with the Mac's own choices); the keys themselves are the core's
/// `global_shortcuts` setting, written as Windows writes them ("ctrl-alt-space", where "alt" is
/// ⌥ and "win" is ⌘).
@MainActor
enum GlobalKeys {
    /// Every action a key can do, with its name in Settings, in the order shown (Windows').
    static let actions: [(id: String, name: String)] = [
        ("play", "Play / Pause"),
        ("next", "Next song"),
        ("previous", "Previous song"),
        ("favorite", "Favorite this song"),
        ("mini", "Show the small player"),
        ("volume_up", "Volume up"),
        ("volume_down", "Volume down"),
    ]

    /// The switch, in the Mac's own defaults.
    static let switchKey = "global-shortcuts"
    static var on: Bool {
        #if DEBUG
        if let testOn { return testOn }
        #endif
        return UserDefaults.standard.bool(forKey: switchKey)
    }

    #if DEBUG
    /// Tests: the switch, without touching the Mac's own defaults (test copies share them).
    static var testOn: Bool?
    static var heldCount: Int { held.count }

    /// Tests: delivers `action`'s hot key event to the app, as the system does when its keys
    /// are pressed (posting real key presses needs Accessibility).
    static func testPress(_ action: String) {
        guard let number = byNumber.first(where: { $0.value == action })?.key else { return }
        var event: EventRef?
        guard CreateEvent(nil, OSType(kEventClassKeyboard), UInt32(kEventHotKeyPressed), 0, 0, &event) == noErr, let event else { return }
        var id = EventHotKeyID(signature: signature, id: number)
        SetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), MemoryLayout<EventHotKeyID>.size, &id)
        SendEventToEventTarget(event, GetApplicationEventTarget())
        ReleaseEvent(event)
    }
    #endif

    /// Opens the mini player (the main window's `openWindow`, set when it shows).
    static var openMini: (() -> Void)?

    /// The actions whose keys another app holds, as found at the last `apply`.
    private(set) static var taken: Set<String> = []
    /// Keys are not held while a row waits for new ones, so pressing held keys records them.
    static var paused = false { didSet { if paused != oldValue { apply() } } }

    private static var held: [EventHotKeyRef] = []
    private static var byNumber: [UInt32: String] = [:]
    private static var handler: EventHandlerRef?
    /// "NDLE": Needle's keys among the app's hot keys.
    private static let signature: OSType = 0x4E44_4C45

    /// The keys chosen for each action (the core's setting), parsed.
    static func chosen(_ settings: NeedleSettings) -> [String: Chord] {
        var keys: [String: Chord] = [:]
        for pair in settings.globalShortcuts where pair.count == 2 {
            if let chord = Chord.parse(pair[1]) { keys[pair[0]] = chord }
        }
        return keys
    }

    /// Holds the keys now chosen (or none, while off or paused).
    static func apply() {
        for ref in held { UnregisterEventHotKey(ref) }
        held = []
        byNumber = [:]
        var busy: Set<String> = []
        if on && !paused {
            installHandler()
            let keys = chosen(Store.shared.settings)
            for (n, action) in actions.enumerated() {
                guard let chord = keys[action.id], let code = chord.keyCode else { continue }
                var ref: EventHotKeyRef?
                let number = UInt32(n + 1)
                let status = RegisterEventHotKey(UInt32(code), chord.carbonModifiers, EventHotKeyID(signature: signature, id: number),
                                                 GetApplicationEventTarget(), 0, &ref)
                if status == noErr, let ref {
                    held.append(ref)
                    byNumber[number] = action.id
                } else {
                    busy.insert(action.id)
                }
            }
        }
        // Rows waiting for keys keep the last word on what is taken.
        if !paused { taken = busy }
    }

    private static func installHandler() {
        guard handler == nil else { return }
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
            var id = EventHotKeyID()
            let status = GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                                           nil, MemoryLayout<EventHotKeyID>.size, nil, &id)
            guard status == noErr, id.signature == 0x4E44_4C45 else { return OSStatus(eventNotHandledErr) }
            let number = id.id
            Task { @MainActor in GlobalKeys.fire(number) }
            return noErr
        }, 1, &spec, nil, &handler)
    }

    private static func fire(_ number: UInt32) {
        guard let action = byNumber[number] else { return }
        run(action)
    }

    /// What a key everywhere does.
    static func run(_ action: String) {
        let store = Store.shared
        switch action {
        case "play": store.toggle()
        case "next": store.next()
        case "previous": store.previous()
        case "favorite":
            if let current = store.current { store.toggleFavorite([current]) }
        case "mini":
            if let mini = NSApp.windows.first(where: { $0.identifier?.rawValue.hasPrefix("mini") == true && $0.isVisible }) {
                mini.close()
            } else {
                openMini?()
            }
        case "volume_up": store.nudgeVolume(0.05)
        case "volume_down": store.nudgeVolume(-0.05)
        default: break
        }
    }

    /// Keeps `keys` for `action` (none: no keys), in the core's setting; the same keys leave any
    /// other action (one action per keys).
    static func set(_ action: String, _ keys: Chord?) {
        let store = Store.shared
        var list = store.settings.globalShortcuts.filter { $0.count == 2 && $0[0] != action }
        if let keys {
            let text = keys.text
            list.removeAll { $0[1] == text }
            list.append([action, text])
        }
        store.set(["global_shortcuts": list])
        apply()
    }

    // MARK: - Keys

    /// Keys pressed together: the modifiers and one key, as Windows writes them ("ctrl-alt-space").
    struct Chord: Equatable {
        var ctrl = false
        var alt = false
        var shift = false
        /// ⌘ (the Windows key on Windows).
        var cmd = false
        var key: String

        /// "ctrl-alt-space" → the keys. None for keys Needle cannot take, or a letter alone (it
        /// needs ⌃, ⌥, or ⌘, unless it is an F key), as on Windows.
        static func parse(_ text: String) -> Chord? {
            let text = text.trimmingCharacters(in: .whitespaces).lowercased()
            let mods: Substring, key: String
            if text.hasSuffix("--") {
                (mods, key) = (text.dropLast(2), "-")
            } else if let dash = text.lastIndex(of: "-") {
                (mods, key) = (text[..<dash], String(text[text.index(after: dash)...]))
            } else {
                (mods, key) = ("", text)
            }
            var chord = Chord(key: key)
            for part in mods.split(separator: "-") {
                switch part {
                case "ctrl", "control": chord.ctrl = true
                case "alt": chord.alt = true
                case "shift": chord.shift = true
                case "win", "super", "cmd", "platform": chord.cmd = true
                default: return nil
                }
            }
            let fKey = chord.key.hasPrefix("f") && chord.key.count > 1
            guard chord.keyCode != nil, chord.ctrl || chord.alt || chord.cmd || fKey else { return nil }
            return chord
        }

        var text: String {
            var parts: [String] = []
            if ctrl { parts.append("ctrl") }
            if alt { parts.append("alt") }
            if shift { parts.append("shift") }
            if cmd { parts.append("win") }
            parts.append(key)
            return parts.joined(separator: "-")
        }

        /// The keys as the Mac shows them: "⌃⌥Space".
        var caps: [String] {
            var caps: [String] = []
            if ctrl { caps.append("⌃") }
            if alt { caps.append("⌥") }
            if shift { caps.append("⇧") }
            if cmd { caps.append("⌘") }
            caps.append(Self.names[key] ?? key.uppercased())
            return caps
        }

        var carbonModifiers: UInt32 {
            var m = 0
            if ctrl { m |= controlKey }
            if alt { m |= optionKey }
            if shift { m |= shiftKey }
            if cmd { m |= cmdKey }
            return UInt32(m)
        }

        var keyCode: Int? { Self.codes[key] }

        /// The keys pressed in a key event, or nil for a key Needle cannot take.
        static func from(_ event: NSEvent) -> Chord? {
            guard let key = codes.first(where: { $0.value == Int(event.keyCode) })?.key else { return nil }
            let flags = event.modifierFlags
            return Chord(ctrl: flags.contains(.control), alt: flags.contains(.option), shift: flags.contains(.shift),
                         cmd: flags.contains(.command), key: key)
        }

        private static let names: [String: String] = [
            "space": "Space", "left": "←", "right": "→", "up": "↑", "down": "↓", "home": "Home", "end": "End",
            "pageup": "Page Up", "pagedown": "Page Down", "insert": "Help", "delete": "⌦", "enter": "↩",
            "tab": "⇥", "backspace": "⌫",
        ]

        /// Key names (Windows' words) and the Mac's key codes.
        static let codes: [String: Int] = {
            var codes: [String: Int] = [
                "a": kVK_ANSI_A, "b": kVK_ANSI_B, "c": kVK_ANSI_C, "d": kVK_ANSI_D, "e": kVK_ANSI_E, "f": kVK_ANSI_F,
                "g": kVK_ANSI_G, "h": kVK_ANSI_H, "i": kVK_ANSI_I, "j": kVK_ANSI_J, "k": kVK_ANSI_K, "l": kVK_ANSI_L,
                "m": kVK_ANSI_M, "n": kVK_ANSI_N, "o": kVK_ANSI_O, "p": kVK_ANSI_P, "q": kVK_ANSI_Q, "r": kVK_ANSI_R,
                "s": kVK_ANSI_S, "t": kVK_ANSI_T, "u": kVK_ANSI_U, "v": kVK_ANSI_V, "w": kVK_ANSI_W, "x": kVK_ANSI_X,
                "y": kVK_ANSI_Y, "z": kVK_ANSI_Z,
                "0": kVK_ANSI_0, "1": kVK_ANSI_1, "2": kVK_ANSI_2, "3": kVK_ANSI_3, "4": kVK_ANSI_4,
                "5": kVK_ANSI_5, "6": kVK_ANSI_6, "7": kVK_ANSI_7, "8": kVK_ANSI_8, "9": kVK_ANSI_9,
                "-": kVK_ANSI_Minus, "=": kVK_ANSI_Equal, ",": kVK_ANSI_Comma, ".": kVK_ANSI_Period, "/": kVK_ANSI_Slash,
                ";": kVK_ANSI_Semicolon, "'": kVK_ANSI_Quote, "[": kVK_ANSI_LeftBracket, "]": kVK_ANSI_RightBracket,
                "\\": kVK_ANSI_Backslash, "`": kVK_ANSI_Grave,
                "space": kVK_Space, "left": kVK_LeftArrow, "right": kVK_RightArrow, "up": kVK_UpArrow, "down": kVK_DownArrow,
                "home": kVK_Home, "end": kVK_End, "pageup": kVK_PageUp, "pagedown": kVK_PageDown, "insert": kVK_Help,
                "delete": kVK_ForwardDelete, "enter": kVK_Return, "tab": kVK_Tab, "backspace": kVK_Delete,
            ]
            let f = [kVK_F1, kVK_F2, kVK_F3, kVK_F4, kVK_F5, kVK_F6, kVK_F7, kVK_F8, kVK_F9, kVK_F10,
                     kVK_F11, kVK_F12, kVK_F13, kVK_F14, kVK_F15, kVK_F16, kVK_F17, kVK_F18, kVK_F19, kVK_F20]
            for (i, code) in f.enumerated() { codes["f\(i + 1)"] = code }
            return codes
        }()
    }
}

/// Settings › Keyboard › Shortcuts everywhere: a switch, then a row for each action with its
/// keys (or "Not set"); click the keys and press new ones ("Press keys…"; Esc stops, ⌫ clears),
/// as on Windows. A row whose keys another app holds says so.
struct GlobalShortcutsSection: View {
    private var store: Store { .shared }
    @AppStorage(GlobalKeys.switchKey) private var stored = false
    private var on: Bool {
        get {
            #if DEBUG
            if let test = GlobalKeys.testOn { return test }
            #endif
            return stored
        }
        nonmutating set { stored = newValue }
    }
    /// The action whose row waits for keys.
    @State private var recording: String?
    @State private var monitor: Any?
    /// Read again after each change, since `GlobalKeys.taken` is not observed.
    @State private var taken: Set<String> = []

    var body: some View {
        Section {
            SettingRow(title: "Shortcuts everywhere", detail: "Keys that work even when Needle is in the background.", icon: "globe") {
                RowToggle(isOn: Binding(get: { on }, set: { on = $0; stopRecording(); GlobalKeys.apply(); taken = GlobalKeys.taken }))
                    .toggleStyle(.switch).labelsHidden()
            }
            if on {
                let keys = GlobalKeys.chosen(store.settings)
                ForEach(GlobalKeys.actions, id: \.id) { action in
                    row(action.id, action.name, keys[action.id])
                }
            }
        } header: {
            GroupHeader(title: "Everywhere")
        }
        .onAppear { taken = GlobalKeys.taken }
        .onDisappear { stopRecording() }
    }

    private func row(_ action: String, _ name: String, _ keys: GlobalKeys.Chord?) -> some View {
        let waiting = recording == action
        return HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                if keys != nil && taken.contains(action) && !waiting {
                    Text("Another app uses these keys. Press other keys.").font(.needle(.callout)).foregroundStyle(store.plainColors.danger)
                }
            }
            .padding(.leading, 34)
            Spacer(minLength: 12)
            Button { waiting ? stopRecording() : record(action) } label: {
                HStack(spacing: 4) {
                    if waiting {
                        cap("Press keys…", accent: true)
                    } else if let keys {
                        ForEach(Array(keys.caps.enumerated()), id: \.offset) { _, c in cap(c, accent: false) }
                    } else {
                        Text("Not set").foregroundStyle(.secondary)
                    }
                }
            }
            .buttonStyle(.plain)
            .help(waiting ? "Press the keys. Esc stops, ⌫ clears." : "Click, then press new keys")
            .accessibilityLabel("\(name): \(waiting ? "press keys" : keys.map { $0.caps.joined() } ?? "not set")")
        }
        .padding(.vertical, 1)
    }

    private func cap(_ text: String, accent: Bool) -> some View {
        Text(text).font(.needle(.callout).monospaced())
            .foregroundStyle(accent ? store.plainColors.accent : .primary)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: Radius.small, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: Radius.small, style: .continuous).strokeBorder(accent ? store.plainColors.accent : .clear))
    }

    /// The next keys pressed in this window go to `action`.
    private func record(_ action: String) {
        stopRecording()
        recording = action
        GlobalKeys.paused = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            guard let waiting = recording else { return event }
            let plain = event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty
            if plain && event.keyCode == UInt16(kVK_Escape) {
                stopRecording()
            } else if plain && (event.keyCode == UInt16(kVK_Delete) || event.keyCode == UInt16(kVK_ForwardDelete)) {
                stopRecording()
                GlobalKeys.set(waiting, nil)
            } else if let chord = GlobalKeys.Chord.from(event).flatMap({ GlobalKeys.Chord.parse($0.text) }) {
                stopRecording()
                GlobalKeys.set(waiting, chord)
            } else {
                store.notify("Use ⌃, ⌥, or ⌘ with it, or an F key.")
            }
            taken = GlobalKeys.taken
            return nil
        }
    }

    private func stopRecording() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        recording = nil
        GlobalKeys.paused = false
        taken = GlobalKeys.taken
    }
}
