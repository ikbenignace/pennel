import MusliKit
import XCTest

final class SilenceWatchTests: XCTestCase {
    func testFlagsSustainedDigitalSilence() {
        var watch = SilenceWatch(threshold: 20)
        XCTAssertFalse(watch.feed([0, 0, 0], at: 10))
        XCTAssertTrue(watch.feed([0, 0], at: 21))
        XCTAssertFalse(watch.feed([0], at: 30), "reports only once")
    }

    func testAnySoundClearsTheWatch() {
        var watch = SilenceWatch(threshold: 20)
        XCTAssertFalse(watch.feed([0, 0.01, 0], at: 1))
        XCTAssertFalse(watch.feed([0, 0], at: 40))
        XCTAssertTrue(watch.heardSound)
    }
}

final class TapBufferTests: XCTestCase {
    func testTapStreamIsTheLastBuffer() {
        XCTAssertEqual(TapBuffers.range(total: 1, interleaved: true, channels: 2), 0..<1)
        XCTAssertEqual(TapBuffers.range(total: 2, interleaved: true, channels: 2), 1..<2, "headset mic comes first")
        XCTAssertEqual(TapBuffers.range(total: 3, interleaved: false, channels: 2), 1..<3)
        XCTAssertNil(TapBuffers.range(total: 1, interleaved: false, channels: 2))
    }
}
