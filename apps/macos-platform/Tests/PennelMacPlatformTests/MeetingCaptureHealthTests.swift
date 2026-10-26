import AVFoundation
import XCTest
@testable import PennelMacPlatform

final class MeetingCaptureHealthTests: XCTestCase {
    func testMicrophoneRecoveryUsesNewAudioEngine() {
        let microphone = MeetingMicrophoneEngine()
        let original = microphone.engine

        microphone.replace()

        XCTAssertFalse(microphone.engine === original)
        XCTAssertFalse(microphone.engine.inputNode.isVoiceProcessingEnabled)
    }

    func testEchoStartupFailureRetriesPlainMicrophone() throws {
        var echoEnabled = true
        var attemptedModes: [Bool] = []

        let echoActive = try MeetingMicrophoneStart.start(
            echoCancellation: true,
            start: {
                attemptedModes.append(echoEnabled)
                if echoEnabled { throw CocoaError(.fileReadUnknown) }
            },
            disableEcho: { echoEnabled = false }
        )

        XCTAssertFalse(echoActive)
        XCTAssertEqual(attemptedModes, [true, false])
    }

    func testPlainMicrophoneStartupDoesNotAttemptEchoFallback() throws {
        var attempts = 0
        let echoRemovals = CapturedSamples()

        let echoActive = try MeetingMicrophoneStart.start(
            echoCancellation: false,
            start: { attempts += 1 },
            disableEcho: { echoRemovals.count += 1 }
        )

        XCTAssertFalse(echoActive)
        XCTAssertEqual(attempts, 1)
        XCTAssertEqual(echoRemovals.count, 0)
    }

    func testFailedPlainMicrophoneRetryPropagatesItsError() {
        var attempts = 0

        XCTAssertThrowsError(try MeetingMicrophoneStart.start(
            echoCancellation: true,
            start: {
                attempts += 1
                throw CocoaError(attempts == 1 ? .fileReadUnknown : .fileWriteUnknown)
            },
            disableEcho: {}
        )) { error in
            XCTAssertEqual((error as? CocoaError)?.code, .fileWriteUnknown)
        }
        XCTAssertEqual(attempts, 2)
    }

    func testFailedMicrophoneRestartRemainsVisibleUntilCaptureActuallyStarts() {
        var health = MeetingCaptureHealth(echoRequested: true)
        health.microphoneStarted(echoActive: true)
        health.microphoneFailed()
        XCTAssertFalse(health.microphoneActive)
        XCTAssertNotNil(health.microphoneError)
        health.microphoneStarting()
        XCTAssertNotNil(health.microphoneError, "a restart attempt is not recovery")
        health.microphoneStarted(echoActive: false)
        XCTAssertTrue(health.microphoneActive)
        XCTAssertNil(health.microphoneError)
        XCTAssertFalse(health.echoActive)
        XCTAssertNotNil(health.echoWarning)
    }

    func testSystemAudioFailureClearsOnlyAfterASuccessfulRestart() {
        var health = MeetingCaptureHealth(echoRequested: false)
        health.systemAudioFailed()
        XCTAssertFalse(health.systemAudioAvailable)
        XCTAssertNotNil(health.systemAudioError)
        XCTAssertTrue(health.bridgeObject(heardSystemAudio: false, archiveError: nil)["terminal_error"] is NSNull)
        health.systemAudioStarting()
        XCTAssertNotNil(health.systemAudioError)
        health.systemAudioStarted()
        XCTAssertTrue(health.systemAudioAvailable)
        XCTAssertNil(health.systemAudioError)
    }

    func testEchoAvailabilityDescribesTheRunningEngineRatherThanThePreference() {
        var health = MeetingCaptureHealth(echoRequested: true)
        XCTAssertFalse(health.echoActive)
        health.microphoneStarted(echoActive: false)
        XCTAssertFalse(health.echoActive)
        XCTAssertNotNil(health.echoWarning)
        health.microphoneStarted(echoActive: true)
        XCTAssertTrue(health.echoActive)
        XCTAssertNil(health.echoWarning)
        health.microphoneFailed()
        XCTAssertFalse(health.echoActive)
    }

    func testHealthBridgeKeepsArchiveFailuresSeparateFromCaptureAvailability() throws {
        var health = MeetingCaptureHealth(echoRequested: false)
        health.microphoneStarted(echoActive: false)
        health.systemAudioStarted()
        let snapshot = health.bridgeObject(heardSystemAudio: true, archiveError: "Audio bewaren mislukt.")
        XCTAssertEqual(snapshot["microphone_active"] as? Bool, true)
        XCTAssertEqual(snapshot["system_audio_available"] as? Bool, true)
        XCTAssertEqual(snapshot["heard_system_audio"] as? Bool, true)
        XCTAssertEqual(snapshot["echo_cancellation"] as? Bool, false)
        XCTAssertEqual(snapshot["archive_error"] as? String, "Audio bewaren mislukt.")
        XCTAssertEqual(snapshot["terminal_error"] as? String, "Audio bewaren mislukt.")
        XCTAssertTrue(snapshot["microphone_error"] is NSNull)
        XCTAssertNoThrow(try JSONSerialization.data(withJSONObject: snapshot))
    }

    func testArchiveFailureIsImmediatelyReadableAndNeverErasesTranscriptAudio() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let received = CapturedSamples()
        let context = Unmanaged.passUnretained(received).toOpaque()
        let sink: AudioSink = { context, _, samples, count, _ in
            guard let context, let samples else { return }
            let received = Unmanaged<CapturedSamples>.fromOpaque(context).takeUnretainedValue()
            received.count += Array(UnsafeBufferPointer(start: samples, count: count)).count
        }
        let writer = try TrackWriter(
            track: 0, sink: sink, context: context, started: 0,
            url: directory.appendingPathComponent("ik.m4a"),
            write: { _ in throw CocoaError(.fileWriteOutOfSpace) }
        )
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: MonoResampler.format, frameCapacity: 16_000))
        buffer.frameLength = 16_000
        buffer.floatChannelData![0].initialize(repeating: 0.1, count: 16_000)
        writer.append(buffer, at: 0)
        XCTAssertNotNil(writer.finish())
        XCTAssertNotNil(writer.archiveError)
        XCTAssertEqual(received.count, 16_000)
    }

    func testLaterArchiveWritesDoNotHideAnEarlierMissingBlock() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        var attempts = 0
        let writer = try TrackWriter(
            track: 0, sink: { _, _, _, _, _ in }, context: nil, started: 0,
            url: directory.appendingPathComponent("ik.m4a"),
            write: { _ in
                attempts += 1
                if attempts == 1 { throw CocoaError(.fileWriteOutOfSpace) }
            }
        )
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: MonoResampler.format, frameCapacity: 16_000))
        buffer.frameLength = 16_000
        buffer.floatChannelData![0].initialize(repeating: 0.1, count: 16_000)
        writer.append(buffer, at: 0)
        writer.append(buffer, at: 1)
        XCTAssertNotNil(writer.finish())
        XCTAssertEqual(attempts, 2)
        XCTAssertNotNil(writer.archiveError)
    }
}

private final class CapturedSamples {
    var count = 0
}
