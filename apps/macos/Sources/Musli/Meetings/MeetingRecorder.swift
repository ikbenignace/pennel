@preconcurrency import AVFoundation
@preconcurrency import MusliCore
import MusliKit
import os

/// Writes one track to an .m4a file and forwards ~1 s batches of 16 kHz samples to the session.
private final class TrackWriter: @unchecked Sendable {
    private let speaker: Speaker
    private let session: MeetingSession
    private let queue: DispatchQueue
    private var file: AVAudioFile?
    private var pending: [Float] = []

    init(speaker: Speaker, session: MeetingSession, url: URL) {
        self.speaker = speaker
        self.session = session
        queue = DispatchQueue(label: "dev.musli.track.\(url.lastPathComponent)")
        file = try? AVAudioFile(
            forWriting: url,
            settings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: 16_000,
                AVNumberOfChannelsKey: 1,
                AVEncoderBitRateKey: 32_000,
            ],
            commonFormat: .pcmFormatFloat32,
            interleaved: false
        )
    }

    func append(_ buffer: AVAudioPCMBuffer) {
        queue.async {
            try? self.file?.write(from: buffer)
            self.pending.append(contentsOf: buffer.monoSamples)
            if self.pending.count >= 16_000 {
                self.session.pushAudio(speaker: self.speaker, samples: self.pending)
                self.pending.removeAll(keepingCapacity: true)
            }
        }
    }

    func finish() {
        queue.sync {
            if !pending.isEmpty { session.pushAudio(speaker: speaker, samples: pending) }
            pending.removeAll()
            file = nil
        }
    }
}

@MainActor
final class MeetingRecorder {
    private let log = Logger(subsystem: "dev.musli.Musli", category: "meeting")
    private let engine = AVAudioEngine()
    private let tap = SystemAudioTap()
    private var me: TrackWriter?
    private var others: TrackWriter?
    private var startedAt: Date?
    private var configObserver: NSObjectProtocol?
    private var outputListener: AudioObjectPropertyListenerBlock?
    private let silence = SilenceProbe()
    private(set) var systemAudioAvailable = true
    private var tapProcesses: (() -> [AudioObjectID])?
    var onSystemAudioSilent: (() -> Void)?

    var elapsed: TimeInterval { startedAt.map { Date().timeIntervalSince($0) } ?? 0 }

    /// `tapProcesses` is re-evaluated on every (re)start so helpers that appear later are included.
    func start(session: MeetingSession, tapProcesses: (() -> [AudioObjectID])? = nil) throws {
        self.tapProcesses = tapProcesses
        let dir = URL(fileURLWithPath: session.dir())
        me = TrackWriter(speaker: .me, session: session, url: dir.appendingPathComponent("ik.m4a"))
        others = TrackWriter(speaker: .others, session: session, url: dir.appendingPathComponent("anderen.m4a"))
        try startMicrophone()
        startSystemAudio()
        // AirPods connecting or switching profile reconfigures the engine and silently stops its tap.
        configObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.restartMicrophone() }
        }
        outputListener = SystemAudioTap.observeOutputChanges { [weak self] in
            Task { @MainActor in self?.restartSystemAudio() }
        }
        startedAt = Date()
    }

    func stop() {
        if let configObserver { NotificationCenter.default.removeObserver(configObserver) }
        if let outputListener { SystemAudioTap.stopObserving(outputListener) }
        configObserver = nil
        outputListener = nil
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        tap.stop()
        me?.finish()
        others?.finish()
        me = nil
        others = nil
        startedAt = nil
        tapProcesses = nil
    }

    private func startMicrophone() throws {
        guard let me else { return }
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard AudioInput.isUsable(sampleRate: format.sampleRate, channels: format.channelCount) else {
            throw NoInputDevice()
        }
        let resampler = MonoResampler(from: format)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { buffer, _ in
            if let converted = resampler.convert(buffer) { me.append(converted) }
        }
        engine.prepare()
        try engine.start()
    }

    private func restartMicrophone() {
        guard startedAt != nil else { return }
        engine.stop()
        do {
            try startMicrophone()
            log.notice("microphone restarted after configuration change")
        } catch {
            log.error("microphone restart failed: \(String(describing: error), privacy: .public)")
        }
    }

    private func startSystemAudio() {
        guard let others else { return }
        silence.reset()
        do {
            let resampler = MonoResampler(from: try tap.prepare(processes: tapProcesses?()))
            let silence = silence
            try tap.start { [weak self] buffer in
                guard let converted = resampler.convert(buffer) else { return }
                others.append(converted)
                if silence.feed(converted.monoSamples) {
                    Task { @MainActor in self?.onSystemAudioSilent?() }
                }
            }
            systemAudioAvailable = true
        } catch {
            log.error("system audio tap failed: \(String(describing: error), privacy: .public)")
            systemAudioAvailable = false
        }
    }

    private func restartSystemAudio() {
        guard startedAt != nil else { return }
        tap.stop()
        startSystemAudio()
        log.notice("system audio restarted after output change")
    }
}

/// Thread-safe wrapper so the IOProc can feed the silence watch.
private final class SilenceProbe: @unchecked Sendable {
    private let lock = NSLock()
    private var watch = SilenceWatch()
    private var samples = 0

    func reset() {
        lock.withLock {
            watch = SilenceWatch()
            samples = 0
        }
    }

    func feed(_ chunk: [Float]) -> Bool {
        lock.withLock {
            samples += chunk.count
            return watch.feed(chunk, at: Double(samples) / 16_000)
        }
    }
}
