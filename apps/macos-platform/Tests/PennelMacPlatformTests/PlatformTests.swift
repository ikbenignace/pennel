import AVFoundation
import AppKit
import ObjectiveC.runtime
import XCTest
@testable import PennelMacPlatform

final class PlatformTests: XCTestCase {
    func testPasteboardRestorePreservesRichFormats() {
        let original = [["public.utf8-plain-text": Data("original".utf8), "public.html": Data("<b>original</b>".utf8)]]
        let board = FakePasteboard(items: original)
        let token = PasteboardGuard.begin(board, text: "dictation")
        PasteboardGuard.end(board, token)
        XCTAssertEqual(board.items, original)
    }

    func testPasteboardRestoreDoesNotOverwriteNewCopy() {
        let board = FakePasteboard(items: [["public.utf8-plain-text": Data("original".utf8)]])
        let token = PasteboardGuard.begin(board, text: "dictation")
        board.setString("new copy")
        PasteboardGuard.end(board, token)
        XCTAssertEqual(board.items, [["public.utf8-plain-text": Data("new copy".utf8)]])
    }

    func testTapUsesLastBuffersAfterAggregateDeviceInput() {
        XCTAssertEqual(TapBuffers.range(total: 4, interleaved: false, channels: 2), 2..<4)
        XCTAssertEqual(TapBuffers.range(total: 2, interleaved: true, channels: 2), 1..<2)
        XCTAssertNil(TapBuffers.range(total: 1, interleaved: false, channels: 2))
    }

    func testSilenceFallbackDoesNotTreatQuietRoomAsBrokenInput() {
        var watch = SilenceWatch(threshold: 5)
        XCTAssertFalse(watch.feed([0, 0], at: 4))
        XCTAssertTrue(watch.feed([0, 0], at: 5))
        XCTAssertFalse(watch.feed([0, 0], at: 6))
        var quiet = SilenceWatch(threshold: 5)
        XCTAssertFalse(quiet.feed([0.000_01], at: 1))
        XCTAssertFalse(quiet.feed([0], at: 20))
    }

    func testPermissionsInspectionHasNoPromptingSideEffects() throws {
        let raw = try XCTUnwrap(permissionsJSON())
        defer { stringFree(raw) }
        let data = Data(String(cString: raw).utf8)
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertNotNil(object["microphone"] as? String)
        XCTAssertNotNil(object["accessibility"] as? String)
        XCTAssertNotNil(object["apple_speech"] as? Bool)
        XCTAssertEqual(object["system_audio"] as? String, "unknown")
    }

    func testOtherInstancesOnlyReportsLegacyOrCanonicalAppsAndExcludesSelf() throws {
        let raw = try XCTUnwrap(otherInstancesJSON())
        defer { stringFree(raw) }
        let data = Data(String(cString: raw).utf8)
        let instances = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [[String: Any]])
        for instance in instances {
            XCTAssertNotEqual(instance["pid"] as? Int32, ProcessInfo.processInfo.processIdentifier)
            XCTAssertTrue(["dev.pennel.Pennel", "dev.musli.Musli"].contains(instance["bundle_id"] as? String ?? ""))
        }
    }

    @MainActor
    func testOverlayUsesANonactivatingPanelWithoutChangingTheHostWindowClass() throws {
        try autoreleasepool {
                _ = NSApplication.shared
                let window = NSWindow(contentRect: NSRect(x: -20_000, y: -20_000, width: 420, height: 72), styleMask: .borderless, backing: .buffered, defer: false)
                window.isReleasedWhenClosed = false
                defer { window.close() }
                let originalClass = NSStringFromClass(object_getClass(window)!)
                let content = try XCTUnwrap(window.contentView)
                let pointer = Unmanaged.passUnretained(window).toOpaque()
                configurePanel(pointer, false)
                let panel = try XCTUnwrap(NSApplication.shared.windows.first { $0 is NSPanel && $0.contentView === content } as? NSPanel)
                XCTAssertTrue(panel.styleMask.contains(.nonactivatingPanel))
                XCTAssertFalse(panel.canBecomeKey)
                XCTAssertFalse(panel.canBecomeMain)
                XCTAssertTrue(panel.ignoresMouseEvents)
                XCTAssertFalse(panel.isVisible)
                configurePanel(pointer, true)
                XCTAssertFalse(panel.ignoresMouseEvents)
                XCTAssertEqual(NSStringFromClass(object_getClass(window)!), originalClass)
                XCTAssertTrue(panel.contentView === content)
                XCTAssertFalse(window.isVisible)
        }
    }

    func testM4AAudioCanBeDecodedForTheSharedRustCore() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let file = directory.appendingPathComponent("anderen.m4a")
        let count = 16_000
        let samples = (0..<count).map { Float(sin(Double($0) * 2 * .pi * 440 / 16_000) * 0.2) }
        try autoreleasepool {
            let writer = try AVAudioFile(forWriting: file, settings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 16_000,
                AVNumberOfChannelsKey: 1,
                AVEncoderBitRateKey: 32_000,
            ], commonFormat: .pcmFormatFloat32, interleaved: false)
            let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: MonoResampler.format, frameCapacity: AVAudioFrameCount(count)))
            buffer.frameLength = AVAudioFrameCount(count)
            samples.withUnsafeBufferPointer { source in buffer.floatChannelData![0].update(from: source.baseAddress!, count: count) }
            try writer.write(from: buffer)
        }
        let decoded = try AudioDecoder.monoSamples(of: file)
        XCTAssertGreaterThan(decoded.count, 15_000)
        XCTAssertLessThan(decoded.count, 18_000)
        XCTAssertTrue(decoded.allSatisfy(\.isFinite))
        XCTAssertGreaterThan(decoded.reduce(0) { $0 + abs($1) }, 500)
    }

    func testTrackArchivesAndCallbacksShareOneTimelineAcrossAnInterruption() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let received = ReceivedFrames()
        let context = Unmanaged.passUnretained(received).toOpaque()
        let sink: AudioSink = { context, track, samples, count, offset in
            guard let context, let samples else { return }
            let received = Unmanaged<ReceivedFrames>.fromOpaque(context).takeUnretainedValue()
            received.frames.append((track, Array(UnsafeBufferPointer(start: samples, count: count)), offset))
        }
        let leftURL = directory.appendingPathComponent("ik.m4a")
        let rightURL = directory.appendingPathComponent("anderen.m4a")
        let left = try TrackWriter(track: 0, sink: sink, context: context, started: 0, url: leftURL)
        let right = try TrackWriter(track: 1, sink: sink, context: context, started: 0, url: rightURL)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: MonoResampler.format, frameCapacity: 16_000))
        buffer.frameLength = 16_000
        buffer.floatChannelData![0].initialize(repeating: 0.1, count: 16_000)
        left.append(buffer, at: 0)
        left.append(buffer, at: 3)
        XCTAssertNil(left.finish())
        right.append(buffer, at: 1)
        right.append(buffer, at: 3)
        XCTAssertNil(right.finish())
        for track in [UInt32(0), UInt32(1)] {
            let frames = received.frames.filter { $0.0 == track }
            XCTAssertEqual(frames.reduce(0) { $0 + $1.1.count }, 64_000)
            var position = 0
            for (_, samples, offset) in frames {
                XCTAssertEqual(offset, UInt64(position) * 1_000 / 16_000)
                position += samples.count
            }
        }
        let leftSamples = try AudioDecoder.monoSamples(of: leftURL)
        let rightSamples = try AudioDecoder.monoSamples(of: rightURL)
        XCTAssertEqual(leftSamples.count, rightSamples.count)
        XCTAssertEqual(leftSamples.count, 64_000)
        let callbackCount = received.frames.count
        left.append(buffer, at: 5)
        right.append(buffer, at: 5)
        XCTAssertNil(left.finish())
        XCTAssertNil(right.finish())
        XCTAssertEqual(received.frames.count, callbackCount, "stopped writers must never call a released Rust context")
    }
}

private final class ReceivedFrames {
    var frames: [(UInt32, [Float], UInt64)] = []
}

private final class FakePasteboard: PasteboardLike {
    var changeCount = 0
    var items: [[String: Data]]
    init(items: [[String: Data]]) { self.items = items }
    func snapshot() -> [[String: Data]] { items }
    func restore(_ items: [[String: Data]]) { self.items = items; changeCount += 1 }
    func setString(_ text: String) { items = [["public.utf8-plain-text": Data(text.utf8)]]; changeCount += 1 }
}
