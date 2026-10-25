import AVFoundation
import XCTest
@testable import PennelMacPlatform

final class LiveSpeechPreviewTests: XCTestCase {
    func testVolatileCorrectionReplacesItsEarlierRange() {
        var caption = LiveSpeechCaptionReducer()
        XCTAssertEqual(caption.update(start: 0, end: 1, text: "Een", isFinal: false), "Een")
        XCTAssertEqual(caption.update(start: 0, end: 2, text: "Een eerste zin", isFinal: false), "Een eerste zin")
        XCTAssertEqual(caption.update(start: 0, end: 2, text: "Een eerste zin.", isFinal: true), "Een eerste zin.")
        XCTAssertEqual(caption.text, "Een eerste zin.")
    }

    func testNewPhraseKeepsCommittedTextWithoutDuplicatingPartials() {
        var caption = LiveSpeechCaptionReducer()
        _ = caption.update(start: 0, end: 2, text: "Hallo wereld.", isFinal: true)
        _ = caption.update(start: 2, end: 3, text: "Dit", isFinal: false)
        XCTAssertEqual(caption.update(start: 2, end: 4, text: "Dit is Pennel.", isFinal: true), "Hallo wereld. Dit is Pennel.")
        XCTAssertNil(caption.update(start: 2, end: 4, text: "Dit is Pennel.", isFinal: true))
    }

    func testCommittedRangeCannotBeOverwrittenByLateVolatileText() {
        var caption = LiveSpeechCaptionReducer()
        _ = caption.update(start: 0, end: 2, text: "Een correcte zin.", isFinal: true)
        XCTAssertNil(caption.update(start: 0, end: 2, text: "Een oude gok", isFinal: false))
        XCTAssertEqual(caption.text, "Een correcte zin.")
    }

    func testFinalRangeReplacesOverlappingVolatileFragments() {
        var caption = LiveSpeechCaptionReducer()
        _ = caption.update(start: 0, end: 1, text: "Een", isFinal: false)
        _ = caption.update(start: 1, end: 2, text: "zin", isFinal: false)
        XCTAssertEqual(caption.update(start: 0, end: 2, text: "Een zin.", isFinal: true), "Een zin.")
        XCTAssertNil(caption.update(start: .nan, end: 3, text: "Ongeldig", isFinal: false))
    }

    func testCaptionTailPreservesUnicodeAndRemainsBounded() {
        var caption = LiveSpeechCaptionReducer(maxCharacters: 12)
        _ = caption.update(start: 0, end: 1, text: "Een lange tekst met 👩🏽‍💻 en café.", isFinal: true)
        XCTAssertLessThanOrEqual(caption.text.count, 12)
        XCTAssertTrue(caption.text.hasSuffix("en café."))
    }

    func testPreparationBufferKeepsOnlyBoundedNewestAudioAndItsOffset() {
        let buffer = LiveSpeechPreviewBuffer(maxSamples: 5)
        XCTAssertTrue(buffer.start())
        XCTAssertTrue(buffer.append([0, 1, 2]))
        XCTAssertTrue(buffer.append([3, 4, 5, 6, 7]))
        let chunk = buffer.drain()
        XCTAssertEqual(chunk?.samples, [3, 4, 5, 6, 7])
        XCTAssertEqual(chunk?.startFrame, 3)
        XCTAssertNil(buffer.drain())
    }

    func testStoppingBeforePreparationRejectsAudioAndCannotRestart() {
        let buffer = LiveSpeechPreviewBuffer(maxSamples: 5)
        XCTAssertTrue(buffer.start())
        _ = buffer.append([1, 2, 3])
        buffer.stop()
        XCTAssertFalse(buffer.append([4]))
        XCTAssertNil(buffer.drain())
        XCTAssertFalse(buffer.start())
    }

    func testStoppedPreviewCanBeAwaitedBeforeStartingWithoutRequestingAccess() async {
        let values = LockedPreviewValues()
        let preview = LiveSpeechPreview(language: "nl") { values.append($0) }
        preview.stop()
        await preview.stopAndWait()
        await preview.stopAndWait()
        preview.feed([0, 0, 0])
        XCTAssertTrue(values.values.isEmpty)
    }

    func testCallbackGateRejectsUpdatesAfterStop() {
        let received = LockedPreviewValues()
        let gate = LiveSpeechCallbackGate { received.append($0) }
        gate.emit("Voor stoppen")
        gate.stop()
        gate.emit("Na stoppen")
        XCTAssertEqual(received.values, ["Voor stoppen"])
    }

    func testStopDrainsAnExecutingCallbackBeforeReturning() {
        let entered = DispatchSemaphore(value: 0)
        let release = DispatchSemaphore(value: 0)
        let stopping = DispatchSemaphore(value: 0)
        let stopped = DispatchSemaphore(value: 0)
        let gate = LiveSpeechCallbackGate { _ in
            entered.signal()
            _ = release.wait(timeout: .now() + 2)
        }
        DispatchQueue.global().async { gate.emit("Een update") }
        XCTAssertEqual(entered.wait(timeout: .now() + 2), .success)
        DispatchQueue.global().async { stopping.signal(); gate.stop(); stopped.signal() }
        XCTAssertEqual(stopping.wait(timeout: .now() + 2), .success)
        XCTAssertEqual(stopped.wait(timeout: .now() + 0.02), .timedOut)
        release.signal()
        XCTAssertEqual(stopped.wait(timeout: .now() + 2), .success)
    }

    func testAudioConversionUsesTheChosenAnalyzerFormat() throws {
        let target = try XCTUnwrap(AVAudioFormat(standardFormatWithSampleRate: 24_000, channels: 1))
        let converter = try LiveSpeechAudioConverter(target: target)
        let samples = (0..<1600).map { Float(sin(Double($0) / 20)) }
        let result = try XCTUnwrap(converter.convert(samples))
        XCTAssertEqual(result.format, target)
        XCTAssertGreaterThan(result.frameLength, 0)
        XCTAssertLessThanOrEqual(result.frameLength, 2500)
    }
}

private final class LockedPreviewValues: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: [String] = []
    var values: [String] { lock.withLock { stored } }
    func append(_ value: String) { lock.withLock { stored.append(value) } }
}
