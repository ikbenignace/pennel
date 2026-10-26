import AVFoundation
import XCTest
@testable import PennelMacPlatform

final class MonoResamplerTests: XCTestCase {
    func testOneResamplerFollowsMicrophoneSampleRateChanges() throws {
        let initialFormat = try format(sampleRate: 48_000, channels: 1)
        let resampler = MonoResampler(from: initialFormat)

        for sampleRate in [48_000.0, 24_000.0, 44_100.0] {
            let input = try buffer(sampleRate: sampleRate, channels: 1)
            let output = try XCTUnwrap(resampler.convert(input), "Conversion failed after changing input to \(sampleRate) Hz.")
            assertUsableMonoOutput(output)
        }
    }

    func testOneResamplerFollowsChannelAndSampleRateChanges() throws {
        let initialFormat = try format(sampleRate: 48_000, channels: 1)
        let resampler = MonoResampler(from: initialFormat)

        for (sampleRate, channels) in [(48_000.0, AVAudioChannelCount(2)), (24_000.0, 1), (44_100.0, 2)] {
            let input = try buffer(sampleRate: sampleRate, channels: channels)
            let output = try XCTUnwrap(resampler.convert(input), "Conversion failed for \(channels) channels at \(sampleRate) Hz.")
            assertUsableMonoOutput(output)
        }
    }

    private func format(sampleRate: Double, channels: AVAudioChannelCount) throws -> AVAudioFormat {
        try XCTUnwrap(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: sampleRate, channels: channels, interleaved: false))
    }

    private func buffer(sampleRate: Double, channels: AVAudioChannelCount) throws -> AVAudioPCMBuffer {
        let format = try format(sampleRate: sampleRate, channels: channels)
        let frames = AVAudioFrameCount(sampleRate / 5)
        let buffer = try XCTUnwrap(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames))
        buffer.frameLength = frames
        let data = try XCTUnwrap(buffer.floatChannelData)
        for channel in 0..<Int(channels) {
            for frame in 0..<Int(frames) {
                data[channel][frame] = Float(sin(Double(frame) * 2 * .pi * 440 / sampleRate)) * Float(channel + 1) * 0.15
            }
        }
        return buffer
    }

    private func assertUsableMonoOutput(_ output: AVAudioPCMBuffer, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertEqual(output.format.sampleRate, 16_000, file: file, line: line)
        XCTAssertEqual(output.format.channelCount, 1, file: file, line: line)
        XCTAssertEqual(output.format.commonFormat, .pcmFormatFloat32, file: file, line: line)
        XCTAssertGreaterThan(output.frameLength, 2_560, "A 200 ms input must retain its duration after resampling.", file: file, line: line)
        XCTAssertLessThanOrEqual(output.frameLength, 3_232, file: file, line: line)
        let samples = output.monoSamples
        XCTAssertTrue(samples.allSatisfy(\.isFinite), file: file, line: line)
        XCTAssertTrue(samples.contains { abs($0) > 0.01 }, "A non-silent input must not become digital silence.", file: file, line: line)
    }
}
