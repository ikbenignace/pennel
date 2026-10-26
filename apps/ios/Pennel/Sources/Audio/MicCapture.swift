import AVFoundation
import PennelMobileKit

/// Captures the microphone as 16 kHz mono Float32 chunks. The tap and the audio session exist only between
/// `start` and `stop`, so nothing is heard while the user is not dictating.
final class MicCapture: @unchecked Sendable {
    enum Failure: Error { case denied, unavailable }

    private let engine = AVAudioEngine()
    private var running = false

    static func requestPermission() async -> Bool {
        await AVAudioApplication.requestRecordPermission()
    }

    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        guard !running else { return }
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playAndRecord, mode: .measurement, options: [.allowBluetoothHFP])
        try session.setActive(true)
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0 else {
            try? session.setActive(false, options: .notifyOthersOnDeactivation)
            throw Failure.unavailable
        }
        let resampler = MonoResampler(from: format)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, _ in
            if let converted = resampler.convert(buffer) { onSamples(converted.monoSamples) }
        }
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            try? session.setActive(false, options: .notifyOthersOnDeactivation)
            throw error
        }
        running = true
    }

    func stop() {
        guard running else { return }
        running = false
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}
