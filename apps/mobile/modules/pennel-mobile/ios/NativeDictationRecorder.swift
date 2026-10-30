import AVFoundation
import Foundation
import PennelMobileKit
import UIKit

@MainActor
final class NativeDictationRecorder {
    enum Phase: String { case idle, listening, processing }
    private(set) var phase = Phase.idle
    private(set) var text = ""
    private(set) var error: String?
    private(set) var errorCode: String?
    private(set) var sessionID: String?
    private(set) var resultSessionID: String?
    private(set) var isPractice = false
    var level: Float { buffer.level }
    var cleanupVerification: CleanupVerificationAvailability? { ownedSession?.cleanupVerificationAvailability() }
    var changed: (() -> Void)?

    private let engine: PennelEngine
    private let capture: AudioCapturing
    private let audio: AudioSessionControlling
    private let requestPermission: @MainActor () async -> Bool
    private let reserveHistory: @Sendable () async throws -> Void
    private let buffer = NativeDictationBuffer()
    private var ownedSession: LiveDictationSession?
    private(set) var previewObserver: NativePreviewObserver?
    private var generation: UInt64 = 0
    private var meetingStart: UInt64?
    private var observations: [NSObjectProtocol] = []
    private var pendingStart: UUID?

    init(engine: PennelEngine, capture: AudioCapturing? = nil, audio: AudioSessionControlling? = nil,
         requestPermission: @escaping @MainActor () async -> Bool = { await MicCapture.requestPermission() },
         reserveHistory: (@Sendable () async throws -> Void)? = nil, observeSystem: Bool = true) {
        self.engine = engine
        self.capture = capture ?? SharedMicrophone.shared.client()
        self.audio = audio ?? SharedAudioSession.shared.client(.dictation)
        self.requestPermission = requestPermission
        self.reserveHistory = reserveHistory ?? { try await Task.detached { try engine.ensureHistoryCapacity() }.value }
        guard observeSystem else { return }
        observations.append(NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  AVAudioSession.InterruptionType(rawValue: raw) == .began else { return }
            Task { @MainActor in
                guard let self else { return }
                if self.pendingStart != nil { self.cancel() }
                else { await self.stop() }
            }
        })
        observations.append(NotificationCenter.default.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.cancel() }
        })
    }

    func start(practice: Bool = false) async throws {
        guard phase == .idle, pendingStart == nil else {
            if practice { throw PennelBridgeError("operation_busy", "Finish the current dictation before starting practice.") }
            return
        }
        let token = UUID()
        pendingStart = token
        sessionID = token.uuidString
        resultSessionID = nil
        isPractice = practice
        text = ""
        defer {
            if pendingStart == token { pendingStart = nil }
            if phase == .idle && sessionID == token.uuidString { sessionID = nil; isPractice = false }
        }
        let allowed = await requestPermission()
        guard pendingStart == token, phase == .idle else { throw PennelError.Cancelled }
        guard allowed else {
            throw PennelBridgeError("microphone_denied", "Allow microphone access in Settings.")
        }
        let engine = engine
        if !practice { try await reserveHistory() }
        guard pendingStart == token, phase == .idle else { throw PennelError.Cancelled }
        generation += 1
        error = nil
        errorCode = nil
        text = ""
        buffer.reset()
        let previewGeneration = generation
        let observer = NativePreviewObserver { [weak self] update in
            guard let self, self.phase == .listening, self.generation == previewGeneration else { return }
            self.text = update.text
            self.changed?()
        }
        previewObserver = observer
        let session = engine.beginDictationSession(observer: observer, practice: practice)
        ownedSession = session
        let meeting = MeetingRuntime.shared
        if meeting.isActive {
            meetingStart = meeting.elapsedMs
            meeting.setDictating(true)
        }
        do {
            try audio.activate()
            let buffer = buffer
            let streamsPreview = session.mode() == .local
            try capture.start { samples in
                buffer.append(samples)
                if streamsPreview { session.feedAudio(samples: samples) }
            }
            phase = .listening
            changed?()
        } catch {
            capture.stop()
            audio.deactivate()
            session.cancel()
            ownedSession = nil
            previewObserver = nil
            endMeetingInterval()
            throw error
        }
    }

    func stop() async {
        guard phase == .listening, let session = ownedSession else { return }
        capture.stop()
        audio.deactivate()
        endMeetingInterval()
        session.stopPreview()
        let request = DictationRequest(samples: buffer.take(), sampleRate: 16_000, appBundleId: nil, keys: Keychain.apiKeys())
        let capturedGeneration = generation
        let capturedPractice = isPractice
        let capturedSessionID = sessionID
        phase = .processing
        changed?()
        let result = await Task.detached { () -> Result<DictationResult, Error> in
            Result { try session.finish(request: request) }
        }.value
        guard generation == capturedGeneration, sessionID == capturedSessionID else { return }
        phase = .idle
        ownedSession = nil
        previewObserver = nil
        switch result {
        case .success(let value):
            text = value.text
            resultSessionID = capturedSessionID
            if !capturedPractice { UIPasteboard.general.setItems([["public.utf8-plain-text": value.text]], options: [.localOnly: true]) }
        case .failure(let value):
            resultSessionID = nil
            error = ErrorText.message(value)
            errorCode = ErrorText.failureCode(value)
        }
        changed?()
    }

    func cancel() {
        generation += 1
        pendingStart = nil
        sessionID = nil
        resultSessionID = nil
        isPractice = false
        ownedSession?.cancel()
        ownedSession = nil
        previewObserver = nil
        capture.stop()
        audio.deactivate()
        buffer.reset()
        endMeetingInterval()
        phase = .idle
        text = ""
        error = nil
        errorCode = nil
        changed?()
    }

    func stopPractice(sessionID requested: String) async throws {
        try requirePracticeSession(requested)
        guard pendingStart == nil else { throw PennelBridgeError("operation_busy", "Practice is still starting.") }
        await stop()
    }

    func cancelPractice(sessionID requested: String) throws {
        try requirePracticeSession(requested)
        cancel()
    }

    private func requirePracticeSession(_ requested: String) throws {
        guard isPractice, !requested.isEmpty, sessionID == requested else {
            throw PennelBridgeError("dictation_session_changed", "This practice session is no longer active. Start practice again.")
        }
    }

    private func endMeetingInterval() {
        guard let start = meetingStart else { return }
        meetingStart = nil
        let meeting = MeetingRuntime.shared
        if meeting.isActive {
            meeting.setDictating(false)
            meeting.markDictation(startMs: start, endMs: meeting.elapsedMs)
        }
    }
}

final class NativePreviewObserver: DictationPreviewObserver, @unchecked Sendable {
    let receive: @MainActor @Sendable (DictationPreviewUpdate) -> Void
    init(_ receive: @escaping @MainActor @Sendable (DictationPreviewUpdate) -> Void) { self.receive = receive }
    func changed(update: DictationPreviewUpdate) { Task { @MainActor in receive(update) } }
}

private final class NativeDictationBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [Float] = []
    private var meter: Float = 0
    var level: Float { lock.withLock { meter } }
    func append(_ chunk: [Float]) {
        lock.withLock {
            samples.append(contentsOf: chunk)
            meter = min(1, chunk.reduce(Float(0)) { max($0, abs($1)) } * 4)
        }
    }
    func take() -> [Float] {
        lock.withLock { defer { samples = []; meter = 0 }; return samples }
    }
    func reset() { lock.withLock { samples = []; meter = 0 } }
}
