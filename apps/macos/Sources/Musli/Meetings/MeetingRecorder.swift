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
    private(set) var systemAudioAvailable = true

    var elapsed: TimeInterval { startedAt.map { Date().timeIntervalSince($0) } ?? 0 }

    func start(session: MeetingSession) throws {
        let dir = URL(fileURLWithPath: session.dir())
        let me = TrackWriter(speaker: .me, session: session, url: dir.appendingPathComponent("ik.m4a"))
        let others = TrackWriter(speaker: .others, session: session, url: dir.appendingPathComponent("anderen.m4a"))
        self.me = me
        self.others = others

        let input = engine.inputNode
        let micFormat = input.outputFormat(forBus: 0)
        guard AudioInput.isUsable(sampleRate: micFormat.sampleRate, channels: micFormat.channelCount) else {
            throw NoInputDevice()
        }
        let micResampler = MonoResampler(from: micFormat)
        input.removeTap(onBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: micFormat) { buffer, _ in
            if let converted = micResampler.convert(buffer) { me.append(converted) }
        }
        engine.prepare()
        try engine.start()

        do {
            let resampler = MonoResampler(from: try tap.prepare())
            try tap.start { buffer in
                if let converted = resampler.convert(buffer) { others.append(converted) }
            }
            systemAudioAvailable = true
        } catch {
            log.error("system audio tap failed: \(String(describing: error), privacy: .public)")
            systemAudioAvailable = false
        }
        startedAt = Date()
    }

    func stop() {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        tap.stop()
        me?.finish()
        others?.finish()
        me = nil
        others = nil
        startedAt = nil
    }
}
