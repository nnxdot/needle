import XCTest

/// Pictures of Home's "Start fast" and of Search before typing (Recent, Rules, Browse by genre
/// and by decade): an iPhone upright, an iPad in landscape. Silent: -stub-player, and nothing
/// is played. The library is the made-up one in the test simulator. Each picture is an
/// attachment of the result bundle.
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
        // A cold start on a busy Mac can take minutes: wait before asking the app anything.
        sleep(20)
        _ = app.staticTexts["test-presses"].waitForExistence(timeout: 180)
        // A first start shows the welcome: "Look Around First" closes it.
        let welcome = app.buttons["Look Around First"]
        if welcome.exists { welcome.tap(); sleep(3) }
        return app
    }

    func testShots() {
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        let device = pad ? "ipad" : "iphone"
        XCUIDevice.shared.orientation = pad ? .landscapeLeft : .portrait

        var app = launch(["-tab", "home"])
        sleep(6)
        shot("\(device)-home")
        // A long press on a tile gives its menu (Open, then the album's own).
        let tile = app.buttons["Play Mixtape"]
        if tile.waitForExistence(timeout: 5) {
            tile.press(forDuration: 1.2)
            sleep(2)
            shot("\(device)-home-menu")
        }
        app.terminate()

        app = launch(["-tab", "search", "-seed-recent"])
        sleep(6)
        shot("\(device)-search-before-typing")
        let decades = app.buttons["Decades"]
        if decades.waitForExistence(timeout: 5) {
            if !decades.isHittable { app.swipeUp(); sleep(2) }
            shot("\(device)-browse-genres")
            decades.tap()
            sleep(2)
            shot("\(device)-browse-decades")
            // Back to the first choice, as a new install has it.
            app.buttons["Genres"].tap()
            sleep(1)
        }
        app.terminate()
    }
}
