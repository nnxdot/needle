import AppKit

/// Keys that work anywhere in Needle's windows except while typing: Space plays or pauses,
/// ← and → seek 10 seconds, Esc closes the big player, and the mouse's side buttons go back
/// and forward.
@MainActor
enum Keys {
    static func install(_ store: @escaping () -> Store?) -> Any? {
        NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .otherMouseDown, .scrollWheel]) { event in
            guard let store = store() else { return event }
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
            // A button or switch with the focus takes Space (lists still play with it).
            let control = window.firstResponder is NSControl && !(window.firstResponder is NSTableView)
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
                if store.immersive { store.setImmersive(false); return nil }
                if store.big { store.big = false; return nil }
                if !typing, !store.search.isEmpty { store.search = ""; return nil }
                return event
            default:
                return event
            }
        }
    }

    /// Lists and tables use ← and → themselves (to open and close rows).
    private static func inTable(_ event: NSEvent) -> Bool {
        guard let responder = event.window?.firstResponder as? NSView else { return false }
        return responder is NSOutlineView
    }
}
