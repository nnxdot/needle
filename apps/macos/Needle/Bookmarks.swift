import AppKit

/// The music folders and songs the listener picked, kept for the next start.
///
/// In Apple's sandbox (the App Store build), Needle may read a folder only while the
/// listener's pick is remembered as a security-scoped bookmark. These bookmarks are kept in
/// the data folder (`bookmarks.plist`: path → bookmark) and opened again at each start, before
/// the library is read. Outside the sandbox they change nothing.
@MainActor
enum Bookmarks {
    private static var file: URL?
    private static var saved: [String: Data] = [:]
    /// Opened for this run, and never closed: Needle reads its music the whole time.
    private static var opened: [String: URL] = [:]

    /// Opens every saved bookmark. Call before the library is read.
    static func start(in dataDir: String) {
        let url = URL(fileURLWithPath: dataDir).appendingPathComponent("bookmarks.plist")
        file = url
        if let data = try? Data(contentsOf: url),
           let list = try? PropertyListDecoder().decode([String: Data].self, from: data) {
            saved = list
        }
        reopen()
        // A drive taken away: what was opened on it is closed and dropped, so the next mount
        // opens it afresh (an access kept from before the drive left was dead, and stood in the
        // way of a new one).
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didUnmountNotification, object: nil, queue: .main) { note in
            guard let volume = (note.userInfo?[NSWorkspace.volumeURLUserInfoKey] as? URL)?.path else { return }
            MainActor.assumeIsolated { driveGone(volume) }
        }
    }

    /// Stops and forgets the access opened on a drive that left; its bookmarks stay, for when
    /// it comes back.
    static func driveGone(_ volume: String) {
        let inside = volume.hasSuffix("/") ? volume : volume + "/"
        for path in opened.keys where path == volume || path.hasPrefix(inside) {
            opened.removeValue(forKey: path)?.stopAccessingSecurityScopedResource()
        }
    }

    /// Opens the saved bookmarks not open yet: at the start, and when a drive is plugged in (one
    /// whose folders were away at the start could not be opened then).
    static func reopen() {
        var changed = false
        for (path, bookmark) in saved where opened[path] == nil {
            var stale = false
            // A bookmark that does not resolve now (a drive that is not plugged in) is kept:
            // it may work when its drive is plugged in, or at the next start.
            guard let url = try? URL(resolvingBookmarkData: bookmark, options: [.withSecurityScope, .withoutUI],
                                     relativeTo: nil, bookmarkDataIsStale: &stale) else { continue }
            if url.startAccessingSecurityScopedResource() { opened[path] = url }
            if stale, let fresh = try? url.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil) {
                saved[path] = fresh
                changed = true
            }
        }
        if changed { write() }
    }

    /// Remembers folders or songs the listener just picked, opened, or dropped. A song in a
    /// folder already remembered needs no bookmark of its own.
    static func remember(_ urls: [URL]) {
        var changed = false
        for url in urls where url.isFileURL {
            let path = url.path
            if saved[path] != nil || saved.keys.contains(where: { path.hasPrefix($0.hasSuffix("/") ? $0 : $0 + "/") }) { continue }
            guard let bookmark = try? url.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil) else { continue }
            saved[path] = bookmark
            changed = true
            if url.startAccessingSecurityScopedResource() { opened[path] = url }
        }
        if changed { write() }
    }

    /// Forgets a music folder the listener stopped reading, and the songs remembered inside it.
    static func forget(_ path: String) {
        let inside = path.hasSuffix("/") ? path : path + "/"
        let gone = saved.keys.filter { $0 == path || $0.hasPrefix(inside) }
        guard !gone.isEmpty else { return }
        for key in gone {
            saved[key] = nil
            opened.removeValue(forKey: key)?.stopAccessingSecurityScopedResource()
        }
        write()
    }

    private static func write() {
        guard let file, let data = try? PropertyListEncoder().encode(saved) else { return }
        try? data.write(to: file, options: .atomic)
    }
}
