import XCTest

/// Drives MobileSafari through the spike probe with system-synthesised touches.
/// Every probe log line is beaconed to serve.py (?beacon=1), tagged with ?run=.
final class ProbeUITests: XCTestCase {
    let base = ProcessInfo.processInfo.environment["PROBE_BASE"] ?? "http://127.0.0.1:8000"
    let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")

    override func setUp() { continueAfterFailure = true }

    func load(_ page: String, _ run: String, _ extra: String = "") {
        safari.open(URL(string: "\(base)/\(page)?beacon=1&moves=1&run=\(run)\(extra)")!)
        _ = safari.wait(for: .runningForeground, timeout: 10)
        let card = cardElement("0-1")
        XCTAssertTrue(card.waitForExistence(timeout: 10), "probe did not render for \(run)")
        Thread.sleep(forTimeInterval: 1.0)
    }

    func cardElement(_ id: String) -> XCUIElement {
        safari.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "Card \(id) ")).firstMatch
    }

    func centre(_ id: String) -> XCUICoordinate {
        cardElement(id).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
    }

    func settle() { Thread.sleep(forTimeInterval: 1.5) }

    func runChecklist(_ page: String, _ tag: String) {
        // 1. Quick swipe up from a card (finger down ~50 ms, then a fast flick).
        load(page, "\(tag)-1-swipe-up-fast")
        let a = centre("0-1")
        a.press(forDuration: 0.05, thenDragTo: a.withOffset(CGVector(dx: 0, dy: -260)),
                withVelocity: XCUIGestureVelocity(1500), thenHoldForDuration: 0)
        settle()

        // 1b. A human-paced swipe up: finger rests ~150 ms before moving at a moderate speed.
        load(page, "\(tag)-1b-swipe-up-human")
        let b = centre("0-1")
        b.press(forDuration: 0.15, thenDragTo: b.withOffset(CGVector(dx: 0, dy: -260)),
                withVelocity: XCUIGestureVelocity(600), thenHoldForDuration: 0)
        settle()

        // 2. Quick swipe left from a card.
        load(page, "\(tag)-2-swipe-left-fast")
        let c = centre("0-1")
        c.press(forDuration: 0.05, thenDragTo: c.withOffset(CGVector(dx: -250, dy: 0)),
                withVelocity: XCUIGestureVelocity(1500), thenHoldForDuration: 0)
        settle()

        // 3. Tap a card.
        load(page, "\(tag)-3-tap")
        cardElement("0-1").tap()
        settle()

        // 4 + 6. Press and hold still 0.6 s, release without moving.
        load(page, "\(tag)-4-hold-release")
        centre("0-1").press(forDuration: 0.6)
        settle()

        // 5. Hold until lifted, then drag diagonally into lane 1, hold there, release.
        load(page, "\(tag)-5-hold-drag")
        let d = centre("0-1")
        d.press(forDuration: 0.6, thenDragTo: d.withOffset(CGVector(dx: 220, dy: 120)),
                withVelocity: XCUIGestureVelocity(300), thenHoldForDuration: 0.4)
        settle()
    }

    /// How long may a finger rest on a card before a swipe still scrolls? (hold is 350 ms)
    func testRestSweep() {
        for rest in [0.20, 0.28, 0.32, 0.40] {
            load("probe.html", "sweep-rest-\(Int(rest * 1000))ms")
            let a = centre("0-1")
            a.press(forDuration: rest, thenDragTo: a.withOffset(CGVector(dx: 0, dy: -260)),
                    withVelocity: XCUIGestureVelocity(1000), thenHoldForDuration: 0)
            settle()
        }
        // Slow start: the finger creeps 6 px (inside the 10 px tolerance) before swiping.
        load("probe.html", "sweep-creep-then-swipe")
        let b = centre("0-1")
        b.press(forDuration: 0.1, thenDragTo: b.withOffset(CGVector(dx: 0, dy: -6)),
                withVelocity: XCUIGestureVelocity(20), thenHoldForDuration: 0)
        settle()
    }

    func grip(_ id: String) -> XCUICoordinate {
        safari.descendants(matching: .any)["grip c\(id)"].firstMatch
            .coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
    }

    /// v3: touch drags only from the grip; the card body scrolls and taps.
    func testProbeV4() { runHandle("probe-v4.html", "v4") }
    func testProbeV6() {
        runHandle("probe-v6.html", "v6")
        load("probe-v6.html", "v6-body-hold-drag-to-lane1")
        let d = centre("0-1")
        d.press(forDuration: 0.7, thenDragTo: d.withOffset(CGVector(dx: 220, dy: 120)),
                withVelocity: XCUIGestureVelocity(300), thenHoldForDuration: 0.4)
        settle()
    }
    func testProbeV5() { runHandle("probe-v5.html", "v5") }
    func testProbeV3() { runHandle("probe-v3.html", "v3") }

    func runHandle(_ p: String, _ tag: String) {
        // Your failing swipe: the thumb rests 0.8 s on the card body, then swipes up.
        load(p, "\(tag)-body-rest800-swipe-up")
        let a = centre("0-1")
        a.press(forDuration: 0.8, thenDragTo: a.withOffset(CGVector(dx: 0, dy: -260)),
                withVelocity: XCUIGestureVelocity(800), thenHoldForDuration: 0)
        settle()
        load(p, "\(tag)-body-fast-swipe-up")
        let b = centre("0-1")
        b.press(forDuration: 0.05, thenDragTo: b.withOffset(CGVector(dx: 0, dy: -260)),
                withVelocity: XCUIGestureVelocity(1500), thenHoldForDuration: 0)
        settle()
        load(p, "\(tag)-body-swipe-left")
        let c = centre("0-1")
        c.press(forDuration: 0.3, thenDragTo: c.withOffset(CGVector(dx: -200, dy: 0)),
                withVelocity: XCUIGestureVelocity(1200), thenHoldForDuration: 0)
        settle()
        load(p, "\(tag)-body-long-press")
        centre("0-1").press(forDuration: 1.2)
        settle()
        load(p, "\(tag)-body-tap")
        cardElement("0-1").tap()
        settle()
        load(p, "\(tag)-grip-drag-to-lane1")
        let g = grip("0-1")
        XCTAssertTrue(safari.descendants(matching: .any)["grip c0-1"].firstMatch.exists, "grip not found")
        g.press(forDuration: 0.05, thenDragTo: g.withOffset(CGVector(dx: 150, dy: 140)),
                withVelocity: XCUIGestureVelocity(300), thenHoldForDuration: 0.4)
        settle()
        load(p, "\(tag)-grip-flick-up")
        let h = grip("0-1")
        h.press(forDuration: 0.05, thenDragTo: h.withOffset(CGVector(dx: 0, dy: -200)),
                withVelocity: XCUIGestureVelocity(1500), thenHoldForDuration: 0)
        settle()
        load(p, "\(tag)-grip-tap")
        grip("0-1").tap()
        settle()
    }

    func testProbeV1() { runChecklist("probe.html", "v1") }
    func testProbeV2() { runChecklist("probe-v2.html", "v2") }
}
