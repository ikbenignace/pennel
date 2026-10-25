import AVFoundation
import CoreMedia
import Foundation
import Speech

final class LiveSpeechPreview: @unchecked Sendable {
    private let language: String?
    private let buffer = LiveSpeechPreviewBuffer(maxSamples: 16_000 * 5)
    private let gate: LiveSpeechCallbackGate
    private let lock = NSLock()
    private var task: Task<Void, Never>?
    private var continuation: AsyncStream<Void>.Continuation?

    init(language: String?, onText: @escaping @Sendable (String) -> Void) {
        self.language = language
        gate = LiveSpeechCallbackGate(onText)
    }

    deinit { stop() }

    @discardableResult
    func start() -> Bool {
        guard #available(macOS 26, *), SFSpeechRecognizer.authorizationStatus() == .authorized else { return false }
        return lock.withLock {
            guard buffer.start() else { return false }
            let input = AsyncStream<Void>.makeStream(bufferingPolicy: .bufferingNewest(1))
            continuation = input.continuation
            let language = language
            let buffer = buffer
            let gate = gate
            task = Task.detached {
                await Self.run(input: input.stream, language: language, buffer: buffer, gate: gate)
            }
            return true
        }
    }

    func feed(_ samples: [Float]) {
        guard !samples.isEmpty else { return }
        lock.withLock {
            if buffer.append(samples) { continuation?.yield(()) }
        }
    }

    func stop() {
        // A foreign callback can own a Rust allocation: finish it before stop returns.
        gate.stop()
        lock.withLock {
            buffer.stop()
            continuation?.finish()
            continuation = nil
            task?.cancel()
        }
    }

    func stopAndWait() async {
        stop()
        let running = lock.withLock { task }
        await running?.value
        lock.withLock { task = nil }
    }

    @available(macOS 26, *)
    private static func run(
        input: AsyncStream<Void>, language: String?, buffer: LiveSpeechPreviewBuffer,
        gate: LiveSpeechCallbackGate
    ) async {
        defer { buffer.stop(); gate.stop() }
        let wanted = Locale(identifier: language ?? Locale.preferredLanguages.first ?? "en")
        guard let locale = await DictationTranscriber.supportedLocale(equivalentTo: wanted),
              !Task.isCancelled, buffer.isActive else { return }
        let transcriber = DictationTranscriber(locale: locale, preset: .progressiveLongDictation)
        let analyzer = SpeechAnalyzer(modules: [transcriber])
        let audio = AsyncStream<AnalyzerInput>.makeStream(bufferingPolicy: .bufferingNewest(8))
        let results = Task {
            var caption = LiveSpeechCaptionReducer()
            do {
                for try await result in transcriber.results {
                    guard !Task.isCancelled, buffer.isActive else { break }
                    if let text = caption.update(
                        start: result.range.start.seconds, end: result.range.end.seconds,
                        text: String(result.text.characters), isFinal: result.isFinal
                    ) { gate.emit(text) }
                }
            } catch {
                // Preview failure does not change the authoritative final transcription.
            }
        }
        do {
            guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]),
                  !Task.isCancelled, buffer.isActive else { throw CancellationError() }
            let converter = try LiveSpeechAudioConverter(target: format)
            try await analyzer.prepareToAnalyze(in: format)
            try Task.checkCancellation()
            try await analyzer.start(inputSequence: audio.stream)
            for await _ in input {
                try Task.checkCancellation()
                guard buffer.isActive else { break }
                if let chunk = buffer.drain(), let converted = try converter.convert(chunk.samples) {
                    audio.continuation.yield(AnalyzerInput(
                        buffer: converted,
                        bufferStartTime: CMTime(value: chunk.startFrame, timescale: 16_000)
                    ))
                }
            }
            audio.continuation.finish()
            if !Task.isCancelled, buffer.isActive {
                try await analyzer.finalizeAndFinishThroughEndOfInput()
            }
        } catch {
            // The regular selected-provider path still owns errors and the final result.
        }
        buffer.stop()
        gate.stop()
        audio.continuation.finish()
        await analyzer.cancelAndFinishNow()
        results.cancel()
        await results.value
    }
}

struct LiveSpeechCaptionReducer {
    private struct Fragment {
        let start: TimeInterval
        let end: TimeInterval
        let text: String
        let isFinal: Bool
    }
    private let maxCharacters: Int
    private var fragments: [Fragment] = []
    private var previous = ""

    init(maxCharacters: Int = 1_024) { self.maxCharacters = max(1, maxCharacters) }

    var text: String {
        var joined = ""
        for fragment in fragments {
            let part = fragment.text.trimmingCharacters(in: .whitespacesAndNewlines)
            if !joined.isEmpty, let first = part.first, !".,!?;:)]}".contains(first) { joined += " " }
            joined += part
        }
        return String(joined.suffix(maxCharacters))
    }

    mutating func update(start: TimeInterval, end: TimeInterval, text: String, isFinal: Bool) -> String? {
        guard start.isFinite, end.isFinite, start >= 0, end >= start,
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        let incoming = Fragment(start: start, end: end, text: String(text.suffix(maxCharacters)), isFinal: isFinal)
        func overlaps(_ fragment: Fragment) -> Bool {
            abs(fragment.start - start) < 0.000_001 || (fragment.start < end && start < fragment.end)
        }
        if !isFinal, fragments.contains(where: { $0.isFinal && overlaps($0) }) { return nil }
        fragments.removeAll { overlaps($0) }
        fragments.append(incoming)
        fragments.sort { $0.start < $1.start }
        if fragments.count > 64 { fragments.removeFirst(fragments.count - 64) }
        let visible = self.text
        guard visible != previous else { return nil }
        previous = visible
        return visible
    }
}

struct LiveSpeechPCMChunk: Sendable {
    let samples: [Float]
    let startFrame: Int64
}

final class LiveSpeechPreviewBuffer: @unchecked Sendable {
    private enum Phase { case waiting, running, stopped }
    private let lock = NSLock()
    private let maxSamples: Int
    private var phase = Phase.waiting
    private var pending: [Float] = []
    private var frames: Int64 = 0

    init(maxSamples: Int) { self.maxSamples = max(1, maxSamples) }
    var isActive: Bool { lock.withLock { phase == .running } }

    func start() -> Bool {
        lock.withLock {
            guard phase == .waiting else { return false }
            phase = .running
            return true
        }
    }

    func append(_ samples: [Float]) -> Bool {
        lock.withLock {
            guard phase == .running, !samples.isEmpty else { return false }
            frames += Int64(samples.count)
            if samples.count >= maxSamples { pending = Array(samples.suffix(maxSamples)) }
            else {
                let excess = max(0, pending.count + samples.count - maxSamples)
                if excess > 0 { pending.removeFirst(excess) }
                pending.append(contentsOf: samples)
            }
            return true
        }
    }

    func drain() -> LiveSpeechPCMChunk? {
        lock.withLock {
            guard phase == .running, !pending.isEmpty else { return nil }
            let chunk = LiveSpeechPCMChunk(samples: pending, startFrame: frames - Int64(pending.count))
            pending.removeAll(keepingCapacity: true)
            return chunk
        }
    }

    func stop() { lock.withLock { phase = .stopped; pending.removeAll(keepingCapacity: false) } }
}

final class LiveSpeechCallbackGate: @unchecked Sendable {
    private let lock = NSRecursiveLock()
    private var callback: (@Sendable (String) -> Void)?

    init(_ callback: @escaping @Sendable (String) -> Void) { self.callback = callback }
    func emit(_ text: String) { lock.withLock { callback?(text) } }
    func stop() { lock.withLock { callback = nil } }
}

final class LiveSpeechAudioConverter {
    private let source: AVAudioFormat
    private let target: AVAudioFormat
    private let converter: AVAudioConverter

    init(target: AVAudioFormat) throws {
        guard let source = AVAudioFormat(standardFormatWithSampleRate: 16_000, channels: 1),
              let converter = AVAudioConverter(from: source, to: target) else { throw LiveSpeechAudioError.format }
        self.source = source
        self.target = target
        self.converter = converter
        converter.primeMethod = .none
    }

    func convert(_ samples: [Float]) throws -> AVAudioPCMBuffer? {
        guard !samples.isEmpty else { return nil }
        guard let input = AVAudioPCMBuffer(pcmFormat: source, frameCapacity: AVAudioFrameCount(samples.count)),
              let channel = input.floatChannelData?[0] else { throw LiveSpeechAudioError.format }
        input.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { channel.update(from: $0.baseAddress!, count: samples.count) }
        let capacity = AVAudioFrameCount(ceil(Double(samples.count) * target.sampleRate / source.sampleRate)) + 256
        guard let output = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: capacity) else { throw LiveSpeechAudioError.format }
        var consumed = false
        var error: NSError?
        converter.convert(to: output, error: &error) { _, status in
            guard !consumed else { status.pointee = .noDataNow; return nil }
            consumed = true
            status.pointee = .haveData
            return input
        }
        if let error { throw error }
        return output.frameLength > 0 ? output : nil
    }
}

private enum LiveSpeechAudioError: Error { case format }
