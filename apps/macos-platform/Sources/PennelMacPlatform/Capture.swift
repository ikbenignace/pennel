@preconcurrency import AVFoundation
import Foundation
import CoreAudio

final class NativeCapture {
    private let engine = AVAudioEngine()
    private let tap = SystemAudioTap()
    private let gate = NSLock()
    private var active = false
    private var source: UInt32 = 0
    private var observer: NSObjectProtocol?
    private var pendingRebuild: DispatchWorkItem?
    private var origin: TimeInterval = 0
    private var sink: AudioSink?
    private var context: UnsafeMutableRawPointer?

    func start(source: UInt32, sink: @escaping AudioSink, context: UnsafeMutableRawPointer?) throws {
        self.source = source
        self.sink = sink
        self.context = context
        origin = ProcessInfo.processInfo.systemUptime
        active = true
        do {
            if source == 0 {
                try startMicrophone()
                observer = NotificationCenter.default.addObserver(
                    forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main
                ) { [weak self] _ in
                    guard let self, self.gate.withLock({ self.active }) else { return }
                    self.engine.stop()
                    try? self.startMicrophone()
                }
            } else {
                try startSystem()
                tap.onConfigurationChange = { [weak self] in
                    guard let self, self.gate.withLock({ self.active }) else { return }
                    self.pendingRebuild?.cancel()
                    let work = DispatchWorkItem { [weak self] in
                        guard let self, self.gate.withLock({ self.active }), !self.tap.configurationUnchangedAndFlowing() else { return }
                        self.tap.stop()
                        try? self.startSystem()
                    }
                    self.pendingRebuild = work
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: work)
                }
            }
        } catch {
            stop()
            throw error
        }
    }

    private func startMicrophone() throws {
        try MicrophoneAccess.ensureGranted()
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard AudioInput.isUsable(sampleRate: format.sampleRate, channels: format.channelCount) else {
            throw NoInputDevice()
        }
        let resampler = MonoResampler(from: format)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 2048, format: format) { [weak self] buffer, _ in
            guard let converted = resampler.convert(buffer) else { return }
            self?.send(converted.monoSamples)
        }
        engine.prepare()
        try MicrophoneAccess.start(engine)
    }

    private func startSystem() throws {
        let resampler = MonoResampler(from: try tap.prepare())
        try tap.start { [weak self] buffer, uptime in
            guard let converted = resampler.convert(buffer) else { return }
            self?.send(converted.monoSamples, capturedAt: uptime)
        }
    }

    private func send(_ samples: [Float], capturedAt: TimeInterval? = nil) {
        gate.withLock {
            guard active, let sink, !samples.isEmpty else { return }
            let elapsed = max(0, (capturedAt ?? (ProcessInfo.processInfo.systemUptime - Double(samples.count) / 16_000)) - origin)
            samples.withUnsafeBufferPointer { buffer in
                sink(context, source, buffer.baseAddress, buffer.count, UInt64(elapsed * 1_000))
            }
        }
    }

    func stop() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        pendingRebuild?.cancel()
        pendingRebuild = nil
        tap.onConfigurationChange = nil
        observer = nil
        gate.withLock { active = false }
        if source == 0 {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
        } else { tap.stop() }
    }
}
