@preconcurrency import AVFoundation

/// Converts any PCM buffer to 16 kHz mono Float32, the format the core transcribes.
final class MonoResampler: @unchecked Sendable {
    static let format = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false)!

    private let converter: AVAudioConverter?

    init(from input: AVAudioFormat) {
        converter = AVAudioConverter(from: input, to: Self.format)
    }

    func convert(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let converter else { return nil }
        let ratio = Self.format.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 32
        guard let out = AVAudioPCMBuffer(pcmFormat: Self.format, frameCapacity: capacity) else { return nil }
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
        return error == nil ? out : nil
    }
}

extension AVAudioPCMBuffer {
    var monoSamples: [Float] {
        guard let channel = floatChannelData else { return [] }
        return Array(UnsafeBufferPointer(start: channel[0], count: Int(frameLength)))
    }
}
