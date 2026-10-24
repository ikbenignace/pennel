import AVFoundation

enum AudioDecoder {
    static let sampleRate = 16_000.0

    /// Whole file as 16 kHz mono Float samples, the format the core's speech models expect.
    /// Diarizing holds the whole track in memory (~230 MB per hour), so longer recordings are skipped.
    static let maxSeconds = 3.0 * 3600

    static func monoSamples(of url: URL) throws -> [Float] {
        let file = try AVAudioFile(forReading: url)
        guard Double(file.length) / file.processingFormat.sampleRate <= maxSeconds else {
            throw CocoaError(.fileReadTooLarge)
        }
        guard let target = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: sampleRate, channels: 1, interleaved: false),
              let converter = AVAudioConverter(from: file.processingFormat, to: target)
        else { throw CocoaError(.fileReadCorruptFile) }
        let chunk: AVAudioFrameCount = 32_768
        var samples: [Float] = []
        samples.reserveCapacity(Int(Double(file.length) * sampleRate / file.processingFormat.sampleRate))
        var finished = false
        while !finished {
            guard let output = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: chunk) else { break }
            var readError: Error?
            var conversionError: NSError?
            let status = converter.convert(to: output, error: &conversionError) { frames, inputStatus in
                // AVAudioFile throws at end of file instead of reading zero frames.
                guard file.framePosition < file.length,
                      let input = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: frames)
                else {
                    inputStatus.pointee = .endOfStream
                    return nil
                }
                do {
                    try file.read(into: input, frameCount: frames)
                } catch {
                    readError = error
                    inputStatus.pointee = .endOfStream
                    return nil
                }
                if input.frameLength == 0 {
                    inputStatus.pointee = .endOfStream
                    return nil
                }
                inputStatus.pointee = .haveData
                return input
            }
            if let readError { throw readError }
            if status == .error { throw conversionError ?? CocoaError(.fileReadCorruptFile) }
            if let data = output.floatChannelData, output.frameLength > 0 {
                samples.append(contentsOf: UnsafeBufferPointer(start: data[0], count: Int(output.frameLength)))
            }
            finished = status == .endOfStream || (status == .inputRanDry && file.framePosition >= file.length)
        }
        return samples
    }
}
