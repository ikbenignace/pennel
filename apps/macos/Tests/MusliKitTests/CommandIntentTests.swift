import MusliKit
import XCTest

final class CommandIntentTests: XCTestCase {
    func testShiftOnlyCountsWhileHolding() {
        var intent = CommandIntent()
        intent.shift(true, keyHeld: false)
        XCTAssertFalse(intent.isCommand)
        intent.shift(true, keyHeld: true)
        XCTAssertTrue(intent.isCommand)
        intent.shift(false, keyHeld: true)
        XCTAssertTrue(intent.isCommand)
        intent.reset()
        XCTAssertFalse(intent.isCommand)
    }

    func testStatusShowsCommand() {
        var status = DictationStatus()
        status.recordingStarted(handsFree: false)
        status.markCommand()
        XCTAssertEqual(status.pill, .command)
        status.recordingSubmitted()
        XCTAssertEqual(status.pill, .working)
    }
}
