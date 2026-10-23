import MusliKit
import XCTest

final class PasteQueueTests: XCTestCase {
    func testQueueReleasesInOrder() {
        let queue = PasteQueue<String>()
        let a = queue.reserve()
        let b = queue.reserve()
        XCTAssertEqual(queue.fulfill(b, with: "2"), [])
        XCTAssertEqual(queue.fulfill(a, with: "1"), ["1", "2"])
    }

    func testQueueSkipsNil() {
        let queue = PasteQueue<String>()
        let a = queue.reserve()
        let b = queue.reserve()
        XCTAssertEqual(queue.fulfill(b, with: "2"), [])
        XCTAssertEqual(queue.fulfill(a, with: nil), ["2"])
    }

    func testLaterReservationsKeepWorking() {
        let queue = PasteQueue<String>()
        XCTAssertEqual(queue.fulfill(queue.reserve(), with: "1"), ["1"])
        XCTAssertEqual(queue.fulfill(queue.reserve(), with: "2"), ["2"])
    }
}
