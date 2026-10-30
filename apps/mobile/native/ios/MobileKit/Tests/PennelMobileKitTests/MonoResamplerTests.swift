@preconcurrency import AVFoundation
import Testing
@testable import PennelMobileKit

struct MonoResamplerTests {
    @Test func resampler_48k_stereo_to_16k_mono_length_and_rms() throws {
        let input = try #require(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000, channels: 2, interleaved: false))
        let resampler = MonoResampler(from: input)
        let chunkFrames = 4_800
        let amplitude: Float = 0.5
        var samples: [Float] = []

        for chunk in 0..<10 {
            let buffer = try #require(AVAudioPCMBuffer(pcmFormat: input, frameCapacity: AVAudioFrameCount(chunkFrames)))
            buffer.frameLength = AVAudioFrameCount(chunkFrames)
            for channel in 0..<2 {
                let data = try #require(buffer.floatChannelData)[channel]
                for index in 0..<chunkFrames {
                    let frame = Float(chunk * chunkFrames + index)
                    data[index] = amplitude * sin(2 * .pi * 440 * frame / 48_000)
                }
            }
            let output = try #require(resampler.convert(buffer))
            #expect(output.format.sampleRate == 16_000 && output.format.channelCount == 1)
            samples += output.monoSamples
        }

        // The converter holds back a little filter latency; one second in must yield about one second out.
        #expect(samples.count <= 16_000 + 64 && samples.count >= 16_000 - 1_500, "count \(samples.count)")

        let rms = (samples.dropFirst(1_600).reduce(0) { $0 + $1 * $1 } / Float(samples.count - 1_600)).squareRoot()
        let expected = amplitude / Float(2).squareRoot()
        #expect(abs(rms - expected) / expected < 0.1, "rms \(rms)")
    }
}
