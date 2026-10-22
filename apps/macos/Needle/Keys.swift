import AppKit

/// Keys that work anywhere in Needle's windows except while typing: Space plays or pauses,
/// ← and → seek 10 seconds, Esc closes the big player, and the mouse's side buttons go back
/// and forward. A click outside the search field, or Esc in it, ends its focus (its text
/// stays), so the keys come back to the app.
@MainActor
enum Keys {
    static func install(_ store: @escaping () -> Store?) -> Any? {
        NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .otherMouseDown, .scrollWheel, .leftMouseDown, .rightMouseDown]) { event in
            guard let store = store() else { return event }
            if event.type == .leftMouseDown || event.type == .rightMouseDown {
                leaveSearch(after: event)
                return event
            }
            if event.type == .scrollWheel {
                return VolumeWheel.handle(event, store: store) ? nil : event
            }
            if event.type == .otherMouseDown {
                // Buttons 4 and 5: back and forward.
                switch event.buttonNumber {
                case 3: store.goBack(); return nil
                case 4: store.goForward(); return nil
                default: return event
                }
            }
            // Only in the main window and the mini player, with no sheet or panel over them: an
            // open panel's column view, a sheet's buttons, and Settings use these keys themselves.
            guard let window = event.window,
                  window.identifier.map({ $0.rawValue.hasPrefix("main") || $0.rawValue.hasPrefix("mini") }) == true,
                  window.attachedSheet == nil, NSApp.modalWindow == nil else { return event }
            // A button with the focus (a sheet's, a menu's) takes Space; lists, sliders, and
            // switches clicked do not.
            let control = window.firstResponder is NSButton
            // The lyric timing editor uses Space and the arrows itself.
            let typing = (window.firstResponder as? NSText) != nil || (window.firstResponder is NSTextView)
                || store.route == .timing || control
            let plain = event.modifierFlags.intersection([.command, .control, .option]).isEmpty
            guard plain else { return event }
            switch event.keyCode {
            case 49 where !typing: // Space
                if store.playback?.hasSong == true { store.toggle() } else { return event }
                return nil
            case 123 where !typing && !inTable(event): // ←
                store.seekBy(-10)
                return nil
            case 124 where !typing && !inTable(event): // →
                store.seekBy(10)
                return nil
            case 53: // Esc
                if store.palette { store.palette = false; return nil }
                // In the search field: leave it, its text kept (a second Esc clears the search).
                if searchEditor(window) != nil { window.makeFirstResponder(nil); return nil }
                if store.immersive { store.setImmersive(false); return nil }
                if store.big { store.big = false; return nil }
                if !typing, !store.search.isEmpty { store.search = ""; return nil }
                return event
            default:
                return event
            }
        }
    }

    /// The search field's editor, while the search field has the focus.
    static func searchEditor(_ window: NSWindow?) -> NSTextView? {
        guard let editor = window?.firstResponder as? NSTextView, editor.delegate is NSSearchField else { return nil }
        return editor
    }

    /// A click outside the search field ends its focus. SwiftUI's pages, rows, and tiles do not
    /// take the focus when clicked, so the field kept it (its ring, and every key typed) until
    /// something else did. After the click: what it focused (a list, a field) keeps that.
    private static func leaveSearch(after event: NSEvent) {
        guard let window = event.window, let field = searchEditor(window)?.delegate as? NSSearchField,
              !field.convert(field.bounds, to: nil).contains(event.locationInWindow) else { return }
        DispatchQueue.main.async {
            if searchEditor(window) != nil { window.makeFirstResponder(nil) }
        }
    }

    /// Lists and tables use ← and → themselves (to open and close rows).
    private static func inTable(_ event: NSEvent) -> Bool {
        guard let responder = event.window?.firstResponder as? NSView else { return false }
        return responder is NSOutlineView
    }
}
