import AVFoundation
import XCTest
@testable import PennelMacPlatform

final class MicrophoneAccessTests: XCTestCase {
    func testOnlyAnAuthorizedStatusCountsAsGranted() {
        XCTAssertEqual(MicrophoneAccessState(.authorized), .granted)
        XCTAssertEqual(MicrophoneAccessState(.notDetermined), .notDetermined)
        XCTAssertEqual(MicrophoneAccessState(.denied), .denied)
        XCTAssertEqual(MicrophoneAccessState(.restricted), .denied)
    }

    func testEngineFailureWithoutAccessAsksForThePermission() {
        XCTAssertEqual(MicrophoneStartError.forEngineFailure(access: .denied), .permissionDenied)
        XCTAssertEqual(MicrophoneStartError.forEngineFailure(access: .notDetermined), .permissionDenied)
        XCTAssertEqual(MicrophoneStartError.forEngineFailure(access: .granted), .failed)
    }

    func testMessagesAreDutchAndNeverExposeCoreAudio() {
        for error in [MicrophoneStartError.permissionDenied, .permissionPending, .failed] {
            let message = error.localizedDescription
            XCTAssertFalse(message.contains("coreaudio"), message)
            XCTAssertFalse(message.contains("-10875"), message)
        }
        XCTAssertTrue(MicrophoneStartError.permissionDenied.localizedDescription.contains("Systeeminstellingen"))
    }
}
