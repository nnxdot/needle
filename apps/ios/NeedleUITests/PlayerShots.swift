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
        let screen = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screen)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        // TEST_RUNNER_SHOTS_DIR=<folder> on the xcodebuild line: each picture is also written
        // there at once (a busy Mac can take very long to finish the result bundle).
        if let folder = ProcessInfo.processInfo.environment["SHOTS_DIR"] {
            try? screen.pngRepresentation.write(to: URL(fileURLWithPath: folder).appendingPathComponent(name + ".png"))
        }
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

    /// The iPhone's immersive player alone (after a change to the iPad's layouts).
    func testPhoneImmersive() {
        XCUIDevice.shared.orientation = .portrait
        let on = ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent"]
        // A first start after a change to the library reads it for a while: the second is kept.
        take("iphone-player-immersive", on, wait: 8)
        take("iphone-player-immersive", on, wait: 8)
    }

    /// The release review's fixes: Play on with the system's AirPlay and Bluetooth row, the
    /// round turning cover in the card, and the Player buttons editor. The looks are put back
    /// (immersive on, a square cover).
    func testReleaseFixes() {
        XCUIDevice.shared.orientation = .portrait
        take("iphone-play-on-airplay", ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent", "-sheet", "speakers"], wait: 10)
        take("iphone-player-card-round", ["-open-player", "-immersive", "0", "-look", "cover", "-cover-shape", "round"], wait: 8)
        take("iphone-player-buttons-editor", ["-immersive", "1", "-cover-shape", "square", "-open-player-buttons"], wait: 8)
    }

    /// Play on alone (a first start after an install can be too slow for the sheet).
    func testPlayOn() {
        XCUIDevice.shared.orientation = .portrait
        take("iphone-play-on-airplay", ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent", "-sheet", "speakers"], wait: 10)
    }

    /// The iPad on its side with the pane open, immersive, alone (a first start reads the
    /// library for a while, and its picture comes too early).
    func testPadPaneOpen() {
        XCUIDevice.shared.orientation = .landscapeLeft
        take("ipad-landscape-pane-open", ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent", "-pane-queue"], wait: 10)
    }

    /// The iPad: standing up and on its side, the pane open and closed, immersive on and off
    /// (card), and More. The looks are put back at the end (immersive on, the pane on Up next).
    func testPad() {
        let on = ["-open-player", "-immersive", "1", "-look", "cover", "-buttons", "accent"]
        let off = ["-open-player", "-immersive", "0", "-look", "cover", "-buttons", "accent"]
        XCUIDevice.shared.orientation = .landscapeLeft
        take("ipad-landscape-pane-open", on + ["-pane-queue"], wait: 8)
        take("ipad-landscape-more", on + ["-pane-queue", "-show-more"], wait: 14)
        take("ipad-landscape-card", off + ["-pane-queue"], wait: 8)
        take("ipad-landscape-pane-closed-card", off + ["-pane-closed"], wait: 8)
        take("ipad-landscape-pane-closed", on + ["-pane-closed"], wait: 8)
        XCUIDevice.shared.orientation = .portrait
        take("ipad-portrait-pane-closed", on + ["-pane-closed"], wait: 8)
        take("ipad-portrait-pane-open", on + ["-pane-queue"], wait: 8)
    }
}
