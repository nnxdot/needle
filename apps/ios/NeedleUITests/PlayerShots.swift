import XCTest

/// Pictures of the new player (now-playing/mix.html, states.html, more.html, buttons.html): an
/// iPhone upright, or an iPad upright and on its side. Silent: -stub-player, and the song is
/// the paused one the test library keeps (nothing is played). Each picture is an attachment of
/// the result bundle. The looks the test sets are put back at its end (immersive on, the cover,
/// Accent), as the test library had them.
final class PlayerShots: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
    }

    private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// Starts the app with these flags, waits for it (a busy Mac can take minutes), and takes
    /// a picture `wait` seconds after the player opened.
    private func take(_ name: String, _ args: [String], wait: UInt32 = 6, then: ((XCUIApplication) -> Void)? = nil) {
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player"] + args
        app.launch()
        sleep(20)
        _ = app.staticTexts["test-presses"].waitForExistence(timeout: 180)
        let welcome = app.buttons["Look Around First"]
        if welcome.exists { welcome.tap(); sleep(3) }
        sleep(wait)
        shot(name)
        then?(app)
        app.terminate()
    }

    func testPhone() {
        XCUIDevice.shared.orientation = .portrait
        let player = ["-open-player", "-immersive", "1", "-look", "cover"]
        take("iphone-player-immersive", player + ["-buttons", "accent"])
        take("iphone-player-card", ["-open-player", "-immersive", "0", "-look", "cover"])
        take("iphone-player-lyrics", ["-open-player", "-immersive", "1", "-look", "lyrics"], wait: 8)
        take("iphone-player-up-next", player + ["-show-queue"], wait: 8)
        take("iphone-player-more", player + ["-show-more"], wait: 8)
        for style in ["glass", "wide", "ring", "square", "trio"] {
            take("iphone-buttons-\(style)", player + ["-buttons", style], wait: 4)
        }
        // Search before typing (Browse's All on one line), at the usual and a large text size.
        take("iphone-search-before-typing", ["-tab", "search", "-seed-recent", "-buttons", "accent"], wait: 4)
        take("iphone-search-large-text", ["-tab", "search", "-seed-recent",
                                          "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityL"], wait: 4) { app in
            app.swipeUp()
            sleep(2)
            self.shot("iphone-search-large-text-2")
        }
    }

    /// The pictures again after a fix: the lyric line, More, and Search's All on one line.
    func testPhoneCheck() {
        XCUIDevice.shared.orientation = .portrait
        let player = ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent"]
        take("iphone-player-immersive", player, wait: 8)
        take("iphone-player-card", ["-open-player", "-immersive", "0", "-look", "cover"], wait: 8)
        take("iphone-player-more", player + ["-show-more"], wait: 8)
        take("iphone-search-before-typing", ["-tab", "search", "-seed-recent"], wait: 4) { app in
            app.swipeUp()
            sleep(2)
            self.shot("iphone-search-before-typing-2")
        }
        take("iphone-search-large-text", ["-tab", "search", "-seed-recent",
                                          "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityL"], wait: 4) { app in
            app.swipeUp()
            sleep(2)
            self.shot("iphone-search-large-text-2")
        }
        // Back to the test library's own look.
        take("iphone-reset", player, wait: 1)
    }

    /// More on an iPad on its side, alone.
    func testPadMore() {
        XCUIDevice.shared.orientation = .landscapeLeft
        take("ipad-landscape-more", ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent", "-pane-queue", "-show-more"], wait: 14)
    }

    func testPad() {
        XCUIDevice.shared.orientation = .portrait
        let player = ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent"]
        take("ipad-portrait-pane-open", player + ["-pane-queue"], wait: 8)
        XCUIDevice.shared.orientation = .landscapeLeft
        take("ipad-landscape-pane-open", player + ["-pane-queue"], wait: 8)
        take("ipad-landscape-pane-closed", player + ["-pane-closed"], wait: 8)
        take("ipad-landscape-card", ["-open-player", "-immersive", "0", "-look", "cover", "-pane-queue"], wait: 8)
        take("ipad-landscape-more", player + ["-pane-queue", "-show-more"], wait: 8)
        XCUIDevice.shared.orientation = .portrait
        take("ipad-portrait-pane-closed", player + ["-pane-closed"], wait: 8)
    }
}
