import XCTest

/// Pictures of Home's "Start fast" and of Search before typing (Recent, Rules, Browse), on an
/// iPhone. Silent: -stub-player, and nothing is played. The library is the made-up one in the
/// test simulator's Documents. Each picture is an attachment of the result bundle.
final class HomeShots: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
    }

    private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func launch(_ args: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player"] + args
        app.launch()
        // A first start shows the welcome: "Look Around First" closes it.
        let welcome = app.buttons["Look Around First"]
        if welcome.waitForExistence(timeout: 20) { welcome.tap() }
        _ = app.staticTexts["test-presses"].waitForExistence(timeout: 60)
        return app
    }

    func testPhone() {
        XCUIDevice.shared.orientation = .portrait
        var app = launch(["-tab", "home"])
        sleep(6)
        shot("iphone-home")
        // A long press on a tile gives its menu (Open, then the album's own).
        let tiles = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Play ' AND label != 'Play '"))
        let tile = tiles.element(boundBy: min(2, max(0, tiles.count - 1)))
        if tile.exists {
            tile.press(forDuration: 1.0)
            sleep(2)
            shot("iphone-home-menu")
        }
        app.terminate()

        app = launch(["-tab", "search", "-seed-recent"])
        sleep(6)
        shot("iphone-search-before-typing")
        app.swipeUp()
        sleep(2)
        shot("iphone-search-before-typing-2")
        app.terminate()
    }
}
