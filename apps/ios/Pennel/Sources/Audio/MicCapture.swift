import AVFoundation
import PennelMobileKit

protocol AudioCapturing: AnyObject, Sendable {
    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws
    func stop()
}

protocol AudioSessionControlling: Sendable {
    func activate() throws
    func deactivate()
}

/// The two ways Pennel uses the shared `AVAudioSession`.
struct SystemAudioSession: AudioSessionControlling {
    enum Purpose {
        /// In-app dictation to the clipboard: Pennel is the only audio while it records.
        case dictation
        /// Keyboard session: stays active in the background, so it mixes with whatever else is playing.
        case keyboardSession
    }

    let purpose: Purpose

    func activate() throws {
        let session = AVAudioSession.sharedInstance()
        switch purpose {
        case .dictation:
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.allowBluetoothHFP])
        case .keyboardSession:
            try session.setCategory(.playAndRecord, mode: .default, options: [.mixWithOthers, .allowBluetoothHFP])
        }
        try session.setActive(true)
    }

    func deactivate() {
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }
}

/// Captures the microphone as 16 kHz mono Float32 chunks. The tap exists only between `start` and `stop`; with an
/// `audioSession` the session is activated and deactivated with it, otherwise the owner manages the session.
final class MicCapture: AudioCapturing, @unchecked Sendable {
    enum Failure: Error { case denied, unavailable }

    private let audioSession: AudioSessionControlling?
    /// A fresh engine per start: after a media-services reset the old one is unusable.
    private var engine: AVAudioEngine?

    init(audioSession: AudioSessionControlling? = SystemAudioSession(purpose: .dictation)) {
        self.audioSession = audioSession
    }

    static func requestPermission() async -> Bool {
        await AVAudioApplication.requestRecordPermission()
    }

    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        guard engine == nil else { return }
        try audioSession?.activate()
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0 else {
            audioSession?.deactivate()
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
            audioSession?.deactivate()
            throw error
        }
        self.engine = engine
    }

    func stop() {
        guard let engine else { return }
        self.engine = nil
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        audioSession?.deactivate()
    }
}
