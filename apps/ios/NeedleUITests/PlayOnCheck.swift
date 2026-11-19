import XCTest

/// Play on, tested as a person uses it, with a timeline. Silent: -stub-player (Play/Pause is
/// only counted). The test reads the app's own log (TEST_RUNNER_SIM_DATA: the simulator's data
/// folder) to know when the library has been read ("Scanned"), and writes pictures and a
/// timeline into TEST_RUNNER_SHOTS_DIR.
final class PlayOnCheck: XCTestCase {
    private var started = Date()
    private var lines: [String] = []

    override func setUp() {
        continueAfterFailure = true
    }

    override func tearDown() {
        let text = lines.joined(separator: "\n") + "\n"
        if let folder = ProcessInfo.processInfo.environment["SHOTS_DIR"] {
            let file = URL(fileURLWithPath: folder).appendingPathComponent("timeline-\(name.split(separator: " ").last?.dropLast() ?? "test").txt")
            try? text.write(to: file, atomically: true, encoding: .utf8)
        }
    }

    private func note(_ what: String) {
        let line = String(format: "%7.1f s  ", Date().timeIntervalSince(started)) + what
        lines.append(line)
        print("PLAYON " + line)
    }

    private func shot(_ name: String) {
        let screen = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screen)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
        if let folder = ProcessInfo.processInfo.environment["SHOTS_DIR"] {
            try? screen.pngRepresentation.write(to: URL(fileURLWithPath: folder).appendingPathComponent(name + ".png"))
        }
        note("picture \(name)")
    }

    /// The app's log: the newest needle.log in the simulator's app folders.
    private func log() -> String? {
        guard let data = ProcessInfo.processInfo.environment["SIM_DATA"] else { return nil }
        let apps = URL(fileURLWithPath: data).appendingPathComponent("Containers/Data/Application")
        let files = ((try? FileManager.default.contentsOfDirectory(at: apps, includingPropertiesForKeys: nil)) ?? [])
            .map { $0.appendingPathComponent("Library/Application Support/Needle/logs/needle.log") }
            .filter { FileManager.default.fileExists(atPath: $0.path) }
        let newest = files.max { modified($0) < modified($1) }
        return newest.flatMap { try? String(contentsOf: $0, encoding: .utf8) }
    }

    private func modified(_ url: URL) -> Date {
        ((try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date) ?? .distantPast
    }

    /// The time of a "Scanned" line written after the app started this time, if there is one.
    private func scanned() -> Date? {
        guard let text = log() else { return nil }
        let format = DateFormatter()
        format.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        format.timeZone = .current
        for line in text.split(separator: "\n").reversed() where line.contains("Scanned") {
            guard let when = format.date(from: String(line.prefix(23))) else { continue }
            return when >= started.addingTimeInterval(-1) ? when : nil
        }
        return nil
    }

    /// Waits until the app's log says the library is read (at most `limit` seconds).
    private func waitForScan(limit: TimeInterval) -> Bool {
        let end = Date().addingTimeInterval(limit)
        while Date() < end {
            if let when = scanned() {
                note(String(format: "log: Scanned (at %.1f s)", when.timeIntervalSince(started)))
                return true
            }
            sleep(5)
        }
        note("log: no Scanned within \(Int(limit)) s")
        return false
    }

    /// Step 2 and 3: the library read first, then Play on from the pill; then the app still
    /// answers (Cancel, Play/Pause, a picture of the player).
    func testPlayOnAfterScan() {
        XCUIDevice.shared.orientation = .portrait
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player"]
        started = Date()
        note("launch")
        app.launch()
        note("launched")
        _ = waitForScan(limit: 600)
        let mini = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Undertow'")).firstMatch
        note("mini player found: \(mini.waitForExistence(timeout: 120))")
        mini.tap()
        note("mini player pressed")
        let playOn = app.buttons["Play on"]
        note("Play on found: \(playOn.waitForExistence(timeout: 120))")
        sleep(3)
        playOn.tap()
        note("Play on pressed")
        let cancel = app.buttons["Cancel"].firstMatch
        note("sheet's Cancel found: \(cancel.waitForExistence(timeout: 120))")
        sleep(6)
        shot("iphone-play-on")
        cancel.tap()
        note("Cancel pressed")
        sleep(3)
        let play = app.buttons["Play"].firstMatch
        note("Play found: \(play.waitForExistence(timeout: 60))")
        play.tap()
        note("Play pressed")
        sleep(3)
        shot("iphone-play-on-after")
        // The press reached the player (counted, no sound): close it and read the count.
        app.buttons["Close the player"].tap()
        sleep(3)
        let presses = app.staticTexts["test-presses"]
        note("presses: \(presses.waitForExistence(timeout: 30) ? presses.label : "not found")")
        app.terminate()
        note("done")
    }

    /// Step 4: Play on open right at start (as the failed tries had it), watched for 5 minutes.
    func testPlayOnAtStart() {
        XCUIDevice.shared.orientation = .portrait
        let app = XCUIApplication()
        app.launchArguments = ["-stub-player", "-open-player", "-immersive", "1", "-look", "cover", "-sheet", "speakers"]
        started = Date()
        note("launch")
        app.launch()
        note("launched")
        // XCTest ends a test when the app's accessibility does not answer (the app still
        // building its first window on a busy Mac): ask it nothing for 90 s.
        sleep(90)
        note("asking the app again")
        // Five minutes from when the app is up (a busy Mac can take minutes to start it).
        let up = Date()
        var scanSeen = false
        var answered = false
        while Date().timeIntervalSince(up) < 300 {
            if !scanSeen, let when = scanned() {
                scanSeen = true
                note(String(format: "log: Scanned (at %.1f s)", when.timeIntervalSince(started)))
            }
            if app.buttons["Cancel"].firstMatch.exists {
                if !answered { note("the sheet answers (Cancel is there)") }
                answered = true
                if scanSeen { break }
            } else {
                note("no answer yet (no Cancel)")
            }
            sleep(10)
        }
        if !scanSeen { note("log: no Scanned within 300 s of the start") }
        shot("iphone-play-on-at-start")
        // Does it work: close the sheet, press Play, and read the count (no sound).
        let cancel = app.buttons["Cancel"].firstMatch
        note("Cancel found: \(cancel.waitForExistence(timeout: 60))")
        cancel.tap()
        note("Cancel pressed")
        sleep(3)
        let play = app.buttons["Play"].firstMatch
        note("Play found: \(play.waitForExistence(timeout: 60))")
        play.tap()
        note("Play pressed")
        sleep(3)
        shot("iphone-play-on-at-start-after")
        app.buttons["Close the player"].tap()
        sleep(3)
        let presses = app.staticTexts["test-presses"]
        note("presses: \(presses.waitForExistence(timeout: 30) ? presses.label : "not found")")
        app.terminate()
        note("done")
    }
}
