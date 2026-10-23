import MusliKit
import XCTest

final class DictationStatusTests: XCTestCase {
    func testSilentDictationHidesPill() {
        var status = DictationStatus()
        status.recordingStarted(handsFree: false)
        status.recordingSubmitted()
        XCTAssertEqual(status.pill, .working)
        status.dictationFinished(notice: nil)
        XCTAssertEqual(status.pill, .hidden)
    }

    func testErrorDuringRecordingKeepsListening() {
        var status = DictationStatus()
        status.recordingStarted(handsFree: false)
        status.recordingSubmitted()
        status.recordingStarted(handsFree: false)
        status.dictationFinished(notice: "Geen verbinding")
        XCTAssertEqual(status.pill, .listening)
    }

    func testExpiredErrorFallsBackToWorking() {
        var status = DictationStatus()
        status.recordingStarted(handsFree: false)
        status.recordingSubmitted()
        status.recordingStarted(handsFree: false)
        status.recordingSubmitted()
        status.dictationFinished(notice: "Geen verbinding")
        XCTAssertEqual(status.pill, .error("Geen verbinding"))
        status.noticeExpired()
        XCTAssertEqual(status.pill, .working)
        status.dictationFinished(notice: nil)
        XCTAssertEqual(status.pill, .hidden)
    }

    func testCancelledRecordingHidesPill() {
        var status = DictationStatus()
        status.recordingStarted(handsFree: true)
        XCTAssertEqual(status.pill, .handsFree)
        status.recordingCancelled()
        XCTAssertEqual(status.pill, .hidden)
    }

    func testStandaloneErrorShowsUntilExpired() {
        var status = DictationStatus()
        status.showNotice("Geen toegang tot de microfoon")
        XCTAssertEqual(status.pill, .error("Geen toegang tot de microfoon"))
        status.noticeExpired()
        XCTAssertEqual(status.pill, .hidden)
    }
}

final class AudioInputTests: XCTestCase {
    func testMissingInputDeviceIsNotUsable() {
        XCTAssertFalse(AudioInput.isUsable(sampleRate: 0, channels: 0))
        XCTAssertFalse(AudioInput.isUsable(sampleRate: 48_000, channels: 0))
        XCTAssertTrue(AudioInput.isUsable(sampleRate: 48_000, channels: 1))
    }
}

final class PasteTimingTests: XCTestCase {
    func testRestoreWaitsLongEnoughForSlowApps() {
        XCTAssertGreaterThanOrEqual(PasteboardGuard.restoreDelay, .milliseconds(600))
    }
}
