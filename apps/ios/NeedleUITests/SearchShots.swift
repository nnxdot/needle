import XCTest

/// Pictures of Search (the "Covers first" page): before typing, the results under All, the
/// Songs chip, and nothing found. Silent (-stub-player). The library is a made-up one put in
/// the test simulator's Documents (the word "harbour" finds an artist, two albums, and four songs; "harbor" finds nothing).
/// Each picture is an attachment of the result bundle. testPhone on an iPhone, testPad on an
/// iPad (landscape).
final class SearchShots: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
    }

    /// A first start shows the welcome: "Look Around First" closes it, and the library is read.
    private func welcome() {
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player"]
        app.launch()
        let button = app.buttons["Look Around First"]
        if button.waitForExistence(timeout: 40) { button.tap() }
        sleep(8)
        app.terminate()
    }

    private func take(_ name: String, _ args: [String], wait: UInt32 = 5) {
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player", "-tab", "search"] + args
        app.launch()
        _ = app.staticTexts["test-presses"].waitForExistence(timeout: 60)
        sleep(wait)
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        app.terminate()
    }

    func testPhone() {
        XCUIDevice.shared.orientation = .portrait
        welcome()
        take("iphone-1-before-typing", ["-seed-recent"], wait: 6)
        take("iphone-2-results-all", ["-search", "harbour"])
        take("iphone-3-songs-chip", ["-search", "harbour", "-search-kind", "songs"])
        take("iphone-4-nothing", ["-search", "harbor"])
    }

    func testPad() {
        XCUIDevice.shared.orientation = .landscapeLeft
        welcome()
        take("ipad-1-results-all", ["-search", "harbour"])
        take("ipad-2-before-typing", ["-seed-recent"], wait: 6)
    }
}
