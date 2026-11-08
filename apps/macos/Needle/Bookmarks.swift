import Foundation

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
        var changed = false
        for (path, bookmark) in saved {
            var stale = false
            // A bookmark that does not resolve now (a drive that is not plugged in) is kept:
            // it may work at the next start.
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
