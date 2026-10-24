import PennelKit
import XCTest

final class HotkeyGestureTests: XCTestCase {
    func testHoldSubmits() {
        var gesture = HotkeyGesture()
        XCTAssertEqual(gesture.handle(.down, at: 0), [.start])
        XCTAssertEqual(gesture.handle(.up, at: 1.0), [.submit])
    }

    func testQuickTapCancels() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        XCTAssertEqual(gesture.handle(.up, at: 0.1), [.cancel])
    }

    func testDoubleTapEntersHandsFreeAndNextPressSubmits() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        _ = gesture.handle(.up, at: 0.1)
        XCTAssertEqual(gesture.handle(.down, at: 0.3), [.start])
        XCTAssertTrue(gesture.isHandsFree)
        XCTAssertEqual(gesture.handle(.up, at: 0.4), [])
        XCTAssertEqual(gesture.handle(.down, at: 5), [.submit])
        XCTAssertFalse(gesture.isHandsFree)
        XCTAssertEqual(gesture.handle(.up, at: 5.1), [])
    }

    func testSlowSecondTapIsNewHold() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        _ = gesture.handle(.up, at: 0.1)
        XCTAssertEqual(gesture.handle(.down, at: 1.0), [.start])
        XCTAssertFalse(gesture.isHandsFree)
    }

    func testMaxDurationSubmits() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        XCTAssertEqual(gesture.tick(at: 300), [])
        XCTAssertEqual(gesture.tick(at: 601), [.submit])
        XCTAssertEqual(gesture.handle(.up, at: 602), [])
    }

    func testRepeatedDownWhileHeldIsIgnored() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        XCTAssertEqual(gesture.handle(.down, at: 0.5), [])
        XCTAssertEqual(gesture.handle(.up, at: 1), [.submit])
    }
}

final class HotkeyInterruptTests: XCTestCase {
    func testOtherKeyWhileHoldingCancels() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        XCTAssertEqual(gesture.interrupt(), [.cancel])
        XCTAssertEqual(gesture.handle(.up, at: 1), [])
        XCTAssertEqual(gesture.handle(.down, at: 1.1), [.start])
        XCTAssertFalse(gesture.isHandsFree)
    }

    func testOtherKeyInHandsFreeIsIgnored() {
        var gesture = HotkeyGesture()
        _ = gesture.handle(.down, at: 0)
        _ = gesture.handle(.up, at: 0.1)
        _ = gesture.handle(.down, at: 0.2)
        _ = gesture.handle(.up, at: 0.3)
        XCTAssertEqual(gesture.interrupt(), [])
        XCTAssertEqual(gesture.handle(.down, at: 3), [.submit])
    }

    func testOtherKeyWhenIdleDoesNothing() {
        var gesture = HotkeyGesture()
        XCTAssertEqual(gesture.interrupt(), [])
    }
}
