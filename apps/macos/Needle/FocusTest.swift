#if DEBUG
import AppKit

/// Test builds: `--show search-click` puts the focus in the search field, clicks a place outside
/// it (sent through the window as real mouse events), and writes who has the focus before and
/// after each click in /tmp/needle-focus.log; then Needle quits. Use a test library:
/// `--data-dir /tmp/...`.
@MainActor
enum FocusTest {
    private static let log: FileHandle? = {
        let path = "/tmp/needle-focus.log"
        FileManager.default.createFile(atPath: path, contents: nil)
        return FileHandle(forWritingAtPath: path)
    }()

    static func note(_ text: String) {
        log?.write(Data("\(text)\n".utf8))
        try? log?.synchronize()
    }

    static func startIfAsked() {
        let arguments = CommandLine.arguments
        guard let i = arguments.firstIndex(of: "--show"), arguments.indices.contains(i + 1), arguments[i + 1] == "search-click" else { return }
        Task { @MainActor in
            let store = Store.shared
            try? await Task.sleep(for: .seconds(4))
            guard let window = NSApp.windows.filter({ $0.isVisible && !($0 is NSPanel) })
                .max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }),
                  let view = window.contentView else { note("no window"); NSApp.terminate(nil); return }
            NSApp.activate(ignoringOtherApps: true)
            window.makeKeyAndOrderFront(nil)
            window.setContentSize(NSSize(width: 1280, height: 820))
            let size = view.bounds.size
            note("window \(Int(size.width))x\(Int(size.height)) sheet: \(window.attachedSheet != nil)")
            // Places outside the field, in window points (from the bottom left).
            let places: [(String, NSPoint)] = [
                ("sidebar item", NSPoint(x: 110, y: size.height - 150)),
                ("sidebar empty space", NSPoint(x: 110, y: 260)),
                ("page: a cover", NSPoint(x: 380, y: size.height - 330)),
                ("page: empty space", NSPoint(x: size.width - 40, y: size.height - 300)),
                ("page: lower", NSPoint(x: size.width / 2, y: 200)),
            ]
            for (name, point) in places {
                store.focusSearch += 1
                try? await Task.sleep(for: .milliseconds(800))
                let before = describe(window)
                for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                    if let event = NSEvent.mouseEvent(with: type, location: point, modifierFlags: [],
                                                      timestamp: ProcessInfo.processInfo.systemUptime,
                                                      windowNumber: window.windowNumber, context: nil,
                                                      eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0) {
                        NSApp.postEvent(event, atStart: false)
                    }
                }
                try? await Task.sleep(for: .milliseconds(800))
                let after = describe(window)
                note("\(name) at \(Int(point.x)),\(Int(point.y)): before \(before) | after \(after) | \(Keys.searchEditor(window) == nil ? "LEFT THE FIELD" : "STILL IN THE FIELD")")
                // Back to Home for the next click.
                store.navigate(.home)
                try? await Task.sleep(for: .milliseconds(500))
            }
            // Esc in the field: out of it, its text kept.
            store.focusSearch += 1
            try? await Task.sleep(for: .milliseconds(800))
            store.search = "test"
            try? await Task.sleep(for: .milliseconds(300))
            let before = describe(window)
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [],
                                                timestamp: ProcessInfo.processInfo.systemUptime,
                                                windowNumber: window.windowNumber, context: nil,
                                                characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}",
                                                isARepeat: false, keyCode: 53) {
                    NSApp.postEvent(event, atStart: false)
                }
            }
            try? await Task.sleep(for: .milliseconds(800))
            note("Esc: before \(before) | after \(describe(window)) | \(Keys.searchEditor(window) == nil ? "LEFT THE FIELD" : "STILL IN THE FIELD") | text \"\(store.search)\"")
            // ⌘F after all that: back in the field.
            store.focusSearch += 1
            try? await Task.sleep(for: .milliseconds(800))
            note("Cmd-F: \(describe(window)) | \(Keys.searchEditor(window) == nil ? "NOT IN THE FIELD" : "IN THE FIELD")")
            store.search = ""
            note("done")
            NSApp.terminate(nil)
        }
    }

    private static func describe(_ window: NSWindow) -> String {
        guard let responder = window.firstResponder else { return "nil" }
        var text = String(describing: type(of: responder))
        if let editor = responder as? NSTextView, let delegate = editor.delegate {
            text += " (editing \(String(describing: type(of: delegate))))"
        }
        return text
    }
}
#endif
