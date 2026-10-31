import PennelMacPlatform
import XCTest

final class NativeGuardTests: XCTestCase {
    func testMissingInputDeviceIsNotUsable() {
        XCTAssertFalse(AudioInput.isUsable(sampleRate: 0, channels: 0))
        XCTAssertFalse(AudioInput.isUsable(sampleRate: 0, channels: 1))
        XCTAssertFalse(AudioInput.isUsable(sampleRate: 48_000, channels: 0))
        XCTAssertTrue(AudioInput.isUsable(sampleRate: 48_000, channels: 1))
    }

    func testClipboardRestoreWaitsForSlowApps() {
        XCTAssertGreaterThanOrEqual(PasteboardGuard.restoreDelay, .milliseconds(600))
    }

    func testDigitalSilenceRequiresNonemptyExactZeros() {
        XCTAssertTrue(SilenceWatch.isDigitalSilence([0, 0, 0]))
        XCTAssertFalse(SilenceWatch.isDigitalSilence([0, 0.00001, 0]))
        XCTAssertFalse(SilenceWatch.isDigitalSilence([]))
    }

    func testSustainedDigitalSilenceReportsOnlyOnce() {
        var watch = SilenceWatch(threshold: 20)
        XCTAssertFalse(watch.feed([0, 0, 0], at: 10))
        XCTAssertTrue(watch.feed([0, 0], at: 21))
        XCTAssertFalse(watch.feed([0], at: 30))
    }

    func testAnySoundDisablesTheSilentCaptureWarning() {
        var watch = SilenceWatch(threshold: 20)
        XCTAssertFalse(watch.feed([0, 0.01, 0], at: 1))
        XCTAssertFalse(watch.feed([0, 0], at: 40))
        XCTAssertTrue(watch.heardSound)
    }
}
