@preconcurrency import AVFoundation
import CoreAudio

typealias AudioSink = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<Float>?, Int, UInt64) -> Void

final class TrackWriter: @unchecked Sendable {
    private let track: UInt32
    private let sink: AudioSink
    private let context: UnsafeMutableRawPointer?
    private let started: TimeInterval
    private let queue: DispatchQueue
    private var file: AVAudioFile?
    private var pending: [Float] = []
    private var sent: UInt64 = 0
    private var pendingStart: UInt64 = 0
    private var archiveFailure: String?
    private let writeOverride: ((AVAudioPCMBuffer) throws -> Void)?
    private let healthLock = NSLock()
    private let enqueueLock = NSLock()
    private var accepting = true
    private var heard = false

    init(track: UInt32, sink: @escaping AudioSink, context: UnsafeMutableRawPointer?, started: TimeInterval, url: URL, write: ((AVAudioPCMBuffer) throws -> Void)? = nil) throws {
        self.track = track
        self.sink = sink
        self.context = context
        self.started = started
        self.writeOverride = write
        self.queue = DispatchQueue(label: "dev.pennel.native-track.\(track)")
        file = try AVAudioFile(forWriting: url, settings: [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: 16_000,
            AVNumberOfChannelsKey: 1,
            AVEncoderBitRateKey: 32_000,
        ], commonFormat: .pcmFormatFloat32, interleaved: false)
    }

    var heardSound: Bool { healthLock.withLock { heard } }
    var archiveError: String? { healthLock.withLock { archiveFailure } }

    func append(_ buffer: AVAudioPCMBuffer) {
        let offset = max(0, ProcessInfo.processInfo.systemUptime - started - Double(buffer.frameLength) / 16_000)
        append(buffer, at: offset)
    }

    func append(_ buffer: AVAudioPCMBuffer, at offset: TimeInterval) {
        let samples = buffer.monoSamples
        enqueueLock.withLock {
            guard accepting else { return }
            if samples.contains(where: { $0 != 0 }) { healthLock.withLock { heard = true } }
            queue.async {
                let desired = UInt64(max(0, offset) * 16_000)
                let position = self.sent + UInt64(self.pending.count)
                // Preserve interruptions on the common session timeline, without expanding tiny callback jitter.
                if desired > position + 1_600 {
                    var remaining = desired - position
                    while remaining > 0 {
                        let count = min(remaining, 32_000)
                        self.appendSamples(Array(repeating: 0, count: Int(count)))
                        remaining -= count
                    }
                }
                self.appendSamples(samples)
            }
        }
    }

    private func appendSamples(_ samples: [Float]) {
        guard !samples.isEmpty else { return }
        if let buffer = AVAudioPCMBuffer(pcmFormat: MonoResampler.format, frameCapacity: AVAudioFrameCount(samples.count)),
           let data = buffer.floatChannelData {
            buffer.frameLength = AVAudioFrameCount(samples.count)
            samples.withUnsafeBufferPointer { source in
                if let base = source.baseAddress { data[0].update(from: base, count: samples.count) }
            }
            do {
                if let writeOverride { try writeOverride(buffer) }
                else { try file?.write(from: buffer) }
            } catch {
                healthLock.withLock { archiveFailure = "De audio-opname kon niet worden bewaard. Controleer de vrije schijfruimte." }
            }
        } else {
            healthLock.withLock { archiveFailure = "Een audioblok kon niet voor de opname worden voorbereid." }
        }
        if pending.isEmpty { pendingStart = sent }
        pending.append(contentsOf: samples)
        if pending.count >= 16_000 { flush() }
    }

    private func flush() {
        guard !pending.isEmpty else { return }
        pending.withUnsafeBufferPointer { buffer in
            sink(context, track, buffer.baseAddress, buffer.count, pendingStart * 1_000 / 16_000)
        }
        sent += UInt64(pending.count)
        pending.removeAll(keepingCapacity: true)
    }

    func finish() -> String? {
        // A callback already in flight may retain this writer after the native handle is released.
        // Close admission before draining so it cannot call the released Rust context afterwards.
        enqueueLock.withLock { accepting = false }
        return queue.sync {
            flush()
            file = nil
            return archiveError
        }
    }
}

struct MeetingCaptureHealth {
    let echoRequested: Bool
    private(set) var microphoneActive = false
    private(set) var microphoneError: String?
    private(set) var systemAudioAvailable = false
    private(set) var systemAudioError: String?
    private(set) var echoActive = false

    var echoWarning: String? {
        echoRequested && microphoneActive && !echoActive
            ? "Echo-onderdrukking is niet actief. Gebruik een koptelefoon."
            : nil
    }

    mutating func microphoneStarting() { microphoneActive = false; echoActive = false }
    mutating func microphoneStarted(echoActive: Bool) {
        microphoneActive = true
        microphoneError = nil
        self.echoActive = echoActive
    }
    mutating func microphoneFailed() {
        microphoneStarting()
        microphoneError = "De microfoonopname is onderbroken. Controleer je microfoon."
    }
    mutating func systemAudioStarting() { systemAudioAvailable = false }
    mutating func systemAudioStarted() { systemAudioAvailable = true; systemAudioError = nil }
    mutating func systemAudioFailed() {
        systemAudioAvailable = false
        systemAudioError = "Systeemgeluid is niet beschikbaar. Alleen de microfoon wordt opgenomen."
    }

    func bridgeObject(heardSystemAudio: Bool, archiveError: String?) -> [String: Any] {
        [
            "microphone_active": microphoneActive,
            "microphone_error": microphoneError as Any? ?? NSNull(),
            "system_audio_available": systemAudioAvailable,
            "system_audio_error": systemAudioError as Any? ?? NSNull(),
            "heard_system_audio": heardSystemAudio,
            "echo_cancellation": echoActive,
            "archive_error": archiveError as Any? ?? NSNull(),
            "capture_warning": echoWarning as Any? ?? NSNull(),
            "terminal_error": (archiveError ?? microphoneError) as Any? ?? NSNull(),
        ]
    }
}

final class NativeMeetingRecorder {
    private let log = Diagnostics("meeting")
    private let engine = AVAudioEngine()
    private let tap = SystemAudioTap()
    private var me: TrackWriter?
    private var others: TrackWriter?
    private var startedAt: Date?
    private var configObserver: NSObjectProtocol?
    private var outputListener: AudioObjectPropertyListenerBlock?
    private let silence = SilenceProbe()
    private let micSilence = SilenceProbe(threshold: 5)
    private var health = MeetingCaptureHealth(echoRequested: true)
    private var tapProcesses: (() -> [AudioObjectID])?
    var onSystemAudioSilent: (() -> Void)?

    var heardSystemAudio: Bool { others?.heardSound ?? false }
    var systemAudioAvailable: Bool { health.systemAudioAvailable }
    var isEchoCancellationEnabled: Bool { health.echoActive }
    var healthSnapshot: [String: Any] {
        health.bridgeObject(heardSystemAudio: heardSystemAudio, archiveError: me?.archiveError ?? others?.archiveError)
    }

    var elapsed: TimeInterval { startedAt.map { Date().timeIntervalSince($0) } ?? 0 }

    /// `tapProcesses` is re-evaluated on every (re)start so helpers that appear later are included.
    private var echoCancellation = true

    /// When the recording began, for mapping wall-clock moments (dictation) onto the meeting timeline.
    var startDate: Date? { startedAt }

    func start(directory: URL, sink: @escaping AudioSink, context: UnsafeMutableRawPointer?, tapProcesses: (() -> [AudioObjectID])? = nil, echoCancellation: Bool = true) throws {
        self.tapProcesses = tapProcesses
        self.echoCancellation = echoCancellation
        health = MeetingCaptureHealth(echoRequested: echoCancellation)
        let started = ProcessInfo.processInfo.systemUptime
        me = try TrackWriter(track: 0, sink: sink, context: context, started: started, url: directory.appendingPathComponent("ik.m4a"))
        others = try TrackWriter(track: 1, sink: sink, context: context, started: started, url: directory.appendingPathComponent("anderen.m4a"))
        do { try startMicrophone() } catch { _ = stop(); throw error }
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

    func stop() -> String? {
        if let configObserver { NotificationCenter.default.removeObserver(configObserver) }
        if let outputListener { SystemAudioTap.stopObserving(outputListener) }
        configObserver = nil
        outputListener = nil
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        disableVoiceProcessing()
        tap.stop()
        let meError = me?.finish()
        let othersError = others?.finish()
        me = nil
        others = nil
        startedAt = nil
        tapProcesses = nil
        return meError ?? othersError
    }

    private func startMicrophone() throws {
        guard let me else { return }
        try MicrophoneAccess.ensureGranted()
        health.microphoneStarting()
        let input = engine.inputNode
        if echoCancellation {
            // Apple's echo canceller removes what the speakers play from the mic; keep other audio at full volume.
            do {
                try input.setVoiceProcessingEnabled(true)
                input.voiceProcessingOtherAudioDuckingConfiguration =
                    AVAudioVoiceProcessingOtherAudioDuckingConfiguration(enableAdvancedDucking: false, duckingLevel: .min)
                // Voice processing is a duplex unit: without a running output side the mic delivers only zeros.
                engine.mainMixerNode.outputVolume = 0
            } catch {
                log.error("voice processing unavailable: \(error.localizedDescription)")
            }
        } else {
            disableVoiceProcessing()
        }
        let format = input.outputFormat(forBus: 0)
        guard AudioInput.isUsable(sampleRate: format.sampleRate, channels: format.channelCount) else {
            throw NoInputDevice()
        }
        let resampler = MonoResampler(from: format)
        let micSilence = micSilence
        micSilence.reset()
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            guard let converted = resampler.convert(buffer) else { return }
            me.append(converted)
            if micSilence.feed(converted.monoSamples) {
                Task { @MainActor in self?.microphoneSilent() }
            }
        }
        engine.prepare()
        try MicrophoneAccess.start(engine)
        guard engine.isRunning else { throw NoInputDevice() }
        health.microphoneStarted(echoActive: input.isVoiceProcessingEnabled)
    }

    private func disableVoiceProcessing() {
        guard engine.inputNode.isVoiceProcessingEnabled else { return }
        do {
            try engine.inputNode.setVoiceProcessingEnabled(false)
        } catch {
            log.error("could not disable voice processing: \(error.localizedDescription)")
        }
    }

    /// Exact zeros from the mic mean the capture path is broken (seen with voice processing);
    /// recording without echo cancellation beats losing the user's side of the meeting.
    private func microphoneSilent() {
        guard echoCancellation, startedAt != nil else {
            log.error("microphone delivers only silence")
            if startedAt != nil { health.microphoneFailed() }
            return
        }
        log.error("microphone silent with voice processing; restarting without echo cancellation")
        echoCancellation = false
        engine.stop()
        disableVoiceProcessing()
        do {
            try startMicrophone()
        } catch {
            health.microphoneFailed()
            log.error("microphone restart failed: \(String(describing: error))")
        }
    }

    private func restartMicrophone() {
        guard startedAt != nil else { return }
        engine.stop()
        do {
            try startMicrophone()
            log.notice("microphone restarted after configuration change")
        } catch {
            health.microphoneFailed()
            log.error("microphone restart failed: \(String(describing: error))")
        }
    }

    private func startSystemAudio() {
        guard let others else { return }
        health.systemAudioStarting()
        silence.reset()
        do {
            let resampler = MonoResampler(from: try tap.prepare(processes: tapProcesses?()))
            let silence = silence
            try tap.start { [weak self] buffer in
                guard let converted = resampler.convert(buffer) else { return }
                others.append(converted)
                if silence.feed(converted.monoSamples) {
                    Task { @MainActor in self?.systemAudioSilent() }
                }
            }
            health.systemAudioStarted()
        } catch {
            log.error("system audio tap failed: \(String(describing: error))")
            health.systemAudioFailed()
        }
    }

    /// Some apps play call audio from another process (FaceTime via avconferenced), so an app-only
    /// tap can stay silent; fall back to all system audio before telling the user.
    private func systemAudioSilent() {
        if tapProcesses != nil {
            log.notice("app tap silent; falling back to global system audio")
            tapProcesses = nil
            restartSystemAudio()
        } else {
            onSystemAudioSilent?()
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
    private let threshold: TimeInterval
    private var watch: SilenceWatch
    private var samples = 0

    init(threshold: TimeInterval = 20) {
        self.threshold = threshold
        watch = SilenceWatch(threshold: threshold)
    }

    func reset() {
        lock.withLock {
            watch = SilenceWatch(threshold: threshold)
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
