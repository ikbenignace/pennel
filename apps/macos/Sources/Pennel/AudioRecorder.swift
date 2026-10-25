import AVFoundation
import PennelKit

struct NoInputDevice: Error {}

/// Records the default input as 16 kHz mono Float32. The engine only runs while recording.
final class AudioRecorder: @unchecked Sendable {
    static let sampleRate: Double = 16_000

    private let engine = AVAudioEngine()
    private let lock = NSLock()
    private var samples: [Float] = []
    private var converter: AVAudioConverter?
    private let outputFormat = AVAudioFormat(
        commonFormat: .pcmFormatFloat32,
        sampleRate: AudioRecorder.sampleRate,
        channels: 1,
        interleaved: false
    )!

    func start(
        onSamples: @escaping @Sendable ([Float]) -> Void = { _ in },
        onLevel: @escaping @Sendable (Float) -> Void
    ) throws {
        lock.withLock { samples.removeAll(keepingCapacity: true) }
        let input = engine.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard AudioInput.isUsable(sampleRate: inputFormat.sampleRate, channels: inputFormat.channelCount) else {
            throw NoInputDevice()
        }
        converter = AVAudioConverter(from: inputFormat, to: outputFormat)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 2048, format: inputFormat) { [weak self] buffer, _ in
            guard let self, let converted = self.convert(buffer) else { return }
            self.lock.withLock { self.samples.append(contentsOf: converted) }
            onSamples(converted)
            onLevel(Self.rms(converted))
        }
        engine.prepare()
        try engine.start()
    }

    func stop() -> [Float] {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        return lock.withLock { samples }
    }

    private func convert(_ buffer: AVAudioPCMBuffer) -> [Float]? {
        guard let converter else { return nil }
        let ratio = outputFormat.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
        guard let out = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity) else { return nil }
        var consumed = false
        var error: NSError?
        converter.convert(to: out, error: &error) { _, status in
            if consumed {
                status.pointee = .noDataNow
                return nil
            }
            consumed = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil, let channel = out.floatChannelData else { return nil }
        return Array(UnsafeBufferPointer(start: channel[0], count: Int(out.frameLength)))
    }

    private static func rms(_ samples: [Float]) -> Float {
        guard !samples.isEmpty else { return 0 }
        return (samples.reduce(0) { $0 + $1 * $1 } / Float(samples.count)).squareRoot()
    }
}
