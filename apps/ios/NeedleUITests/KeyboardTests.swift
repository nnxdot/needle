import XCTest

/// The floating player bar and the full-screen player after the software keyboard: both use
/// the whole window again once the keyboard is gone, however it went (an iPad in landscape, a
/// wide window). Runs with -stub-player and needs a song in the player (a paused session in the
/// test library) and the software keyboard (no hardware keyboard on the simulator).
final class KeyboardTests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = true
        XCUIDevice.shared.orientation = .landscapeLeft
        app = XCUIApplication()
        app.launchArguments = ["-stub-player"]
    }

    override func tearDown() {
        app?.terminate()
    }

    private func wait(_ element: XCUIElement, _ seconds: TimeInterval = 15, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertTrue(element.waitForExistence(timeout: seconds), "\(element) never came", file: file, line: line)
    }

    private func waitGone(_ element: XCUIElement, _ seconds: TimeInterval = 8) -> Bool {
        let gone = NSPredicate(format: "exists == false")
        return XCTWaiter.wait(for: [expectation(for: gone, evaluatedWith: element)], timeout: seconds) == .completed
    }

    private var window: CGRect { app.windows.firstMatch.frame }

    /// The floating bar's Previous button (only the bar has one while the player is shut).
    private var barButton: XCUIElement { app.buttons["Previous"].firstMatch }

    /// Points between the floating bar's foot and the window's foot (14 of padding and the
    /// bar's own shape when it is where it belongs).
    private func barGap() -> CGFloat {
        window.maxY - barButton.frame.maxY
    }

    private func keyboardUp() -> Bool { app.keyboards.firstMatch.waitForExistence(timeout: 5) }

    private func keyboardGone() -> Bool { waitGone(app.keyboards.firstMatch, 6) }

    private func sidebar(_ name: String) {
        let cell = app.cells[name].firstMatch
        if !cell.exists, app.buttons["Show Sidebar"].exists { app.buttons["Show Sidebar"].tap() }
        wait(cell, 5)
        cell.tap()
    }

    /// Types in Search with the software keyboard up.
    private func typeInSearch() {
        sidebar("Search")
        let field = app.searchFields.firstMatch
        wait(field)
        field.tap()
        field.typeText("b")
        XCTAssertTrue(keyboardUp(), "the software keyboard is up (the simulator's hardware keyboard must be off)")
    }

    /// The bar is at the window's foot: its Previous button within 60 points of it (42 where it
    /// belongs: the 14 points under the bar and the bar's own half height).
    private func checkBar(_ step: String, file: StaticString = #filePath, line: UInt = #line) {
        sleep(1)
        let gap = barGap()
        print("NEEDLE-TEST \(step): bar gap \(gap), window \(window)")
        XCTAssertLessThan(gap, 60, "\(step): the bar is \(gap) pt above the window's foot", file: file, line: line)
    }

    /// The player uses the whole window: its lowest control is in the lower part of it.
    private func checkPlayer(_ step: String, file: StaticString = #filePath, line: UInt = #line) {
        let close = app.buttons["Close the player"].firstMatch
        wait(close)
        sleep(2)
        shot("player-\(step)")
        let lowest = app.buttons.allElementsBoundByIndex
            .filter { $0.exists && $0.isHittable && $0.frame.minY > close.frame.maxY }
            .map(\.frame.maxY).max() ?? 0
        print("NEEDLE-TEST \(step): player lowest control \(lowest), window \(window)")
        XCTAssertGreaterThan(lowest, window.minY + window.height * 0.62,
                             "\(step): the player's controls stop at \(lowest) of \(window.height)", file: file, line: line)
        close.tap()
        _ = waitGone(close)
    }

    func testBarAndPlayerAfterKeyboard() {
        app.launch()
        wait(barButton, 60)
        checkBar("start")
        shot("start")

        // 1. Typing in Search, then Home from the sidebar while the keyboard is up.
        typeInSearch()
        shot("search-keyboard-up")
        print("NEEDLE-TEST keyboard up: bar gap \(barGap())")
        sidebar("Home")
        XCTAssertTrue(keyboardGone(), "the keyboard went with Search")
        shot("home-after-search")
        checkBar("home after Search")
        barButton.firstMatch.tap()
        // The bar's screen opens the player: the song's name in the middle of the bar.
        if !app.buttons["Close the player"].waitForExistence(timeout: 3) {
            app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        }
        checkPlayer("after-search-home")

        // 2. The keyboard hidden by Search's own Cancel / the keyboard's dismiss key.
        typeInSearch()
        if app.keyboards.buttons["Hide keyboard"].exists {
            app.keyboards.buttons["Hide keyboard"].tap()
        } else if app.buttons["Cancel"].exists {
            app.buttons["Cancel"].tap()
        }
        XCTAssertTrue(keyboardGone(), "the keyboard went with its own key")
        checkBar("search after hide key")
        shot("search-after-hide")

        // 3. The player opened while the keyboard is up.
        typeInSearch()
        app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        checkPlayer("opened-with-keyboard")
        // Search's field takes the keyboard back when the player closes: the bar rides on it
        // while it is up, and is at the foot once it has gone.
        if app.keyboards.firstMatch.exists {
            print("NEEDLE-TEST keyboard back after the player: bar gap \(barGap())")
            sidebar("Home")
            _ = keyboardGone()
        }
        checkBar("after player opened over the keyboard")

        // 4. Away and back while the keyboard is up.
        typeInSearch()
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(2)
        if app.keyboards.firstMatch.exists, app.keyboards.buttons["Hide keyboard"].exists { app.keyboards.buttons["Hide keyboard"].tap() }
        sidebar("Home")
        _ = keyboardGone()
        checkBar("home after background")
        shot("home-after-background")
        app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        checkPlayer("after-background")

        // 5. Turned while the keyboard is up, and back.
        typeInSearch()
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        sidebar("Home")
        _ = keyboardGone()
        checkBar("home after turning")
        shot("home-after-turn")
    }

    /// The player with no pane beside the cover (the reporter's picture), opened in landscape
    /// and turned while open: its controls reach the lower part of the window each time.
    func testPlayerTurns() {
        app.launch()
        wait(barButton, 60)
        app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        let pane = app.buttons["Close the pane"].firstMatch
        if pane.waitForExistence(timeout: 5) { pane.tap() }
        sleep(2)
        shot("player-nopane-landscape")
        checkPlayerStays("landscape, no pane")
        XCUIDevice.shared.orientation = .portrait
        sleep(3)
        shot("player-nopane-portrait")
        checkPlayerStays("turned to portrait")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(3)
        shot("player-nopane-turned-back")
        checkPlayerStays("turned back to landscape")
    }

    /// As `checkPlayer`, leaving the player open.
    private func checkPlayerStays(_ step: String, file: StaticString = #filePath, line: UInt = #line) {
        let close = app.buttons["Close the player"].firstMatch
        wait(close)
        let lowest = app.buttons.allElementsBoundByIndex
            .filter { $0.exists && $0.isHittable && $0.frame.minY > close.frame.maxY }
            .map(\.frame.maxY).max() ?? 0
        print("NEEDLE-TEST \(step): player lowest control \(lowest), window \(window)")
        XCTAssertGreaterThan(lowest, window.minY + window.height * 0.62,
                             "\(step): the player's controls stop at \(lowest) of \(window.height)", file: file, line: line)
    }

    /// Another app's keyboard (Settings' search) while Needle is away, then back in Needle.
    func testOtherAppKeyboard() {
        app.launch()
        wait(barButton, 60)
        checkBar("start")
        let settings = XCUIApplication(bundleIdentifier: "com.apple.Preferences")

        // 1. Settings' keyboard up, then straight back to Needle.
        settings.activate()
        let field = settings.searchFields.firstMatch
        wait(field)
        field.tap()
        field.typeText("wi")
        print("NEEDLE-TEST settings keyboard up: \(settings.keyboards.firstMatch.exists)")
        shot("settings-keyboard-up")
        app.activate()
        sleep(2)
        shot("needle-after-settings-keyboard")
        checkBar("back from Settings with its keyboard up")
        app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        checkPlayer("after-settings-keyboard")

        // 2. Needle's own keyboard up, away to Settings, back.
        typeInSearch()
        settings.activate()
        sleep(2)
        shot("settings-over-needle-keyboard")
        app.activate()
        sleep(2)
        if app.keyboards.buttons["Hide keyboard"].exists { app.keyboards.buttons["Hide keyboard"].tap() }
        sidebar("Home")
        _ = keyboardGone()
        checkBar("home after Settings over Needle's keyboard")
        shot("home-after-settings-over-keyboard")
        app.buttons.matching(NSPredicate(format: "label CONTAINS ', '")).firstMatch.tap()
        checkPlayer("after-settings-over-keyboard")

        // 3. Settings' keyboard up, hidden there, then back.
        settings.activate()
        let again = settings.searchFields.firstMatch
        wait(again)
        again.tap()
        again.typeText("b")
        if settings.keyboards.buttons["Hide keyboard"].exists { settings.keyboards.buttons["Hide keyboard"].tap() }
        else if settings.buttons["Cancel"].exists { settings.buttons["Cancel"].tap() }
        sleep(1)
        app.activate()
        sleep(2)
        checkBar("back after Settings hid its keyboard")
        shot("needle-after-settings-hid")
        settings.terminate()
    }

    private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
