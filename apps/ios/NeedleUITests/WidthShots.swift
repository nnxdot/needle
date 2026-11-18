import XCTest

/// Pictures of the pages that show songs as a wide window's table, for a look at narrow
/// columns: Search's Top Result and Songs, an album, an artist, a playlist, Songs, Favorites,
/// History, and the player's Up Next. Silent (-stub-player). Each picture is an attachment of
/// the result bundle. NEEDLE_SHOTS picks the set: "search" (default) or "all".
final class WidthShots: XCTestCase {
    private struct Shot {
        let name: String
        let args: [String]
        var portrait = false
        var sidebarClosed = false
        var big = false
    }

    override func setUp() {
        continueAfterFailure = true
    }

    private func take(_ shot: Shot) {
        XCUIDevice.shared.orientation = shot.portrait ? .portrait : .landscapeLeft
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player"] + shot.args
        if shot.big { app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityM"] }
        app.launch()
        _ = app.staticTexts["test-presses"].waitForExistence(timeout: 60)
        sleep(4)
        if shot.sidebarClosed {
            let hide = app.buttons.matching(NSPredicate(format: "label == 'Hide Sidebar' OR identifier == 'ToggleSidebar'")).firstMatch
            if hide.exists { hide.tap(); sleep(2) }
        }
        sleep(2)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = shot.name
        attachment.lifetime = .keepAlways
        add(attachment)
        app.terminate()
    }

    func testShots() {
        let search = ["-tab", "search", "-search", "b"]
        var shots = [
            Shot(name: "search-land-sidebar", args: search),
            Shot(name: "search-land-nosidebar", args: search, sidebarClosed: true),
            Shot(name: "search-portrait", args: search, portrait: true),
        ]
        if ProcessInfo.processInfo.environment["NEEDLE_SHOTS"] == "all" {
            for (name, args) in [("songs", ["-place", "songs"]), ("favorites", ["-place", "favorites"]), ("history", ["-place", "history"]),
                                 ("album", ["-album", "Night Swim"]), ("artist", ["-artist", "Coral Avenue"]),
                                 ("playlist", ["-place", "playlist:Road Trip"]), ("queue", ["-show-queue"])] {
                shots.append(Shot(name: "\(name)-land-sidebar", args: args))
                shots.append(Shot(name: "\(name)-land-nosidebar", args: args, sidebarClosed: true))
                shots.append(Shot(name: "\(name)-portrait", args: args, portrait: true))
            }
            shots.append(Shot(name: "search-land-big", args: search, big: true))
            shots.append(Shot(name: "songs-land-big", args: ["-place", "songs"], big: true))
            shots.append(Shot(name: "album-land-big", args: ["-album", "Night Swim"], big: true))
        }
        for shot in shots { take(shot) }
    }
}
