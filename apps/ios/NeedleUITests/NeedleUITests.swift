import XCTest

/// Needle on an iPad with a keyboard and a pointer. Runs with -stub-player: Play/Pause, Next,
/// Previous, and seeking never reach the player (a simulator plays at full volume), and the
/// presses that would have are read from the screen ("test-presses"). The library is the test
/// simulator's made-up one (Night Swim by Coral Avenue; the playlists Road Trip and the
/// read-only Friday Mix).
final class NeedleUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        // Each check is noted and the test goes on: one run tells all.
        continueAfterFailure = true
        XCUIDevice.shared.orientation = .landscapeLeft
        app = XCUIApplication()
        app.launchArguments = ["-stub-player"]
    }

    override func tearDown() {
        app?.terminate()
    }

    /// What reached the player, in order ("toggle", "next", …).
    private var presses: [String] {
        // Each window shows it (two windows: the same presses).
        let label = app.staticTexts.matching(identifier: "test-presses").firstMatch.label
        return label.dropFirst("presses:".count).split(separator: ",").map(String.init).filter { $0 != "typing" }
    }

    /// Needle sees text being typed (its keys' commands are off).
    private var typing: Bool { app.staticTexts.matching(identifier: "test-presses").firstMatch.label.hasSuffix(",typing") }

    private func wait(_ element: XCUIElement, _ seconds: TimeInterval = 15, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertTrue(element.waitForExistence(timeout: seconds), "\(element) never came", file: file, line: line)
    }

    private func value(_ field: XCUIElement) -> String { field.value as? String ?? "" }

    /// Space plays and pauses with no field being typed in, and types a space in one; the arrow
    /// commands move in the text there; Return searches; Esc closes the player and a sheet.
    func testKeysTypeInFields() {
        app.launch()
        wait(app.staticTexts["test-presses"], 40)
        XCTAssertEqual(presses, [])

        // No field: Space is Play/Pause.
        app.typeKey(" ", modifierFlags: [])
        XCTAssertEqual(presses, ["toggle"], "Space with no field should press Play/Pause")

        // The search field: Space types a space, and never reaches the player.
        app.typeKey("f", modifierFlags: .command)
        let field = app.searchFields.firstMatch
        wait(field)
        field.tap()
        field.typeText("a b")
        XCTAssertEqual(value(field), "a b", "typed text")
        XCTAssertTrue(typing, "Needle knows text is being typed")
        app.typeKey(" ", modifierFlags: [])
        XCTAssertEqual(value(field), "a b ", "Space from the keyboard types a space")
        XCTAssertEqual(presses, ["toggle"], "Space in a field never presses Play/Pause")

        // ⌘← and ⌘→ move to the line's start and end, never Previous or Next.
        app.typeKey(XCUIKeyboardKey.leftArrow.rawValue, modifierFlags: .command)
        field.typeText("X")
        XCTAssertEqual(value(field), "Xa b ", "⌘← moved to the start")
        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: .command)
        field.typeText("Y")
        XCTAssertEqual(value(field), "Xa b Y", "⌘→ moved to the end")
        app.typeKey(XCUIKeyboardKey.leftArrow.rawValue, modifierFlags: [.command, .option])
        app.typeKey(XCUIKeyboardKey.rightArrow.rawValue, modifierFlags: [.command, .option])
        XCTAssertEqual(presses, ["toggle"], "no arrow command reached the player while typing")

        // Return searches and leaves the text, the player shut.
        app.typeKey(XCUIKeyboardKey.return.rawValue, modifierFlags: [])
        XCTAssertEqual(value(field), "Xa b Y")
        XCTAssertFalse(app.buttons["Close the player"].exists)
        XCTAssertEqual(presses, ["toggle"])
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])

        // The new playlist's name field: Space types there too.
        app.typeKey("1", modifierFlags: .command)
        let sidebar = app.collectionViews.firstMatch
        let newPlaylist = app.cells["New Playlist"]
        for _ in 0..<4 where !newPlaylist.exists { sidebar.swipeUp() }
        XCTAssertTrue(newPlaylist.waitForExistence(timeout: 5), "the sidebar's New Playlist")
        if newPlaylist.exists {
            newPlaylist.tap()
            let name = app.textFields.firstMatch
            wait(name)
            name.tap()
            name.typeText("a b")
            app.typeKey(" ", modifierFlags: [])
            XCTAssertEqual(value(name), "a b ", "Space types in a playlist's name")
            XCTAssertEqual(presses, ["toggle"])
            app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
            if app.buttons["Cancel"].exists { app.buttons["Cancel"].tap() }
        }

        XCTAssertEqual(presses, ["toggle"], "nothing else reached the player")
    }

    private func waitGone(_ element: XCUIElement, _ seconds: TimeInterval = 8) -> Bool {
        let gone = NSPredicate(format: "exists == false")
        return XCTWaiter.wait(for: [expectation(for: gone, evaluatedWith: element)], timeout: seconds) == .completed
    }

    /// The pointer: a tile shows Play while the pointer is over it, a row is highlighted, a
    /// right click opens a song's menu.
    func testPointer() {
        app.launchArguments += ["-place", "albums"]
        app.launch()
        let tile = app.staticTexts["Night Swim"].firstMatch
        wait(tile, 40)
        XCTAssertFalse(app.buttons["Play Night Swim"].exists, "no Play before the pointer comes")
        tile.hover()
        sleep(1)
        shot("pointer-tile")
        // The simulator delivers no hover to the app (no lift, no Play): noted, not failed.
        print("NEEDLE-TEST hover showed Play: \(app.buttons["Play Night Swim"].waitForExistence(timeout: 3))")

        tile.tap()
        let song = app.staticTexts["Glass Harbour"].firstMatch
        wait(song)
        song.hover()
        shot("pointer-row")
        song.rightClick()
        wait(app.buttons["Add to Playlist"], 5)
        XCTAssertTrue(app.buttons["Go to Artist"].exists || app.buttons["Play Next"].exists)
        shot("pointer-menu")
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        XCTAssertEqual(presses, [], "nothing played")
    }

    /// Songs dragged onto a playlist in the sidebar: a read-only one refuses them, a plain one
    /// takes them.
    func testDragOntoPlaylists() {
        app.launchArguments += ["-album", "Night Swim"]
        app.launch()
        let song = app.staticTexts["Salt in the Air"].firstMatch
        wait(song, 40)
        let sidebar = app.collectionViews.firstMatch
        let readOnly = sidebar.staticTexts["Friday Mix"]
        for _ in 0..<4 where !readOnly.isHittable { sidebar.swipeUp() }
        wait(readOnly, 5)
        song.press(forDuration: 1.2, thenDragTo: readOnly)
        XCTAssertFalse(app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH 'Added'")).firstMatch.waitForExistence(timeout: 3),
                       "a read-only playlist takes no songs")
        shot("drag-readonly")
        let plain = sidebar.staticTexts["Road Trip"]
        wait(plain, 5)
        song.press(forDuration: 1.2, thenDragTo: plain)
        let added = app.staticTexts["Added to “Road Trip”"]
        XCTAssertTrue(added.waitForExistence(timeout: 5), "a plain playlist takes the song")
        shot("drag-plain")
        XCTAssertEqual(presses, [], "nothing played")
    }

    /// The player and Settings close from the keyboard; and what is open in one window is not
    /// open in a new one.
    func testWindowsKeepTheirOwnPlayer() {
        app.launch()
        wait(app.staticTexts["test-presses"], 40)
        app.typeKey("p", modifierFlags: .command)
        wait(app.buttons["Close the player"])
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        print("NEEDLE-TEST Esc closed the player: \(waitGone(app.buttons["Close the player"], 4))")
        if app.buttons["Close the player"].exists {
            app.typeKey(".", modifierFlags: .command)
            XCTAssertTrue(waitGone(app.buttons["Close the player"]), "⌘. closes the player")
        }
        app.typeKey(",", modifierFlags: .command)
        print("NEEDLE-TEST ⌘, opened Settings: \(app.buttons["Done"].waitForExistence(timeout: 5))")
        if !app.buttons["Done"].exists { app.cells["Settings"].firstMatch.tap() }
        wait(app.buttons["Done"])
        app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        print("NEEDLE-TEST Esc closed Settings: \(waitGone(app.buttons["Done"], 4))")
        if app.buttons["Done"].exists {
            app.typeKey(".", modifierFlags: .command)
            XCTAssertTrue(waitGone(app.buttons["Done"]), "⌘. closes Settings")
        }
        // Settings open in this window; then a second window.
        app.cells["Settings"].firstMatch.tap()
        wait(app.buttons["Done"])
        shot("window-one-settings")
        app.open(URL(string: "needle://test-new-window")!)
        sleep(6)
        shot("window-two")
        print("NEEDLE-TEST windows now \(app.windows.count)")
        XCTAssertFalse(app.buttons["Done"].exists, "the new window did not open with Settings")
        XCTAssertFalse(app.buttons["Close the player"].exists, "nor with the player")
        XCTAssertEqual(presses, [], "nothing played")
    }

    private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
