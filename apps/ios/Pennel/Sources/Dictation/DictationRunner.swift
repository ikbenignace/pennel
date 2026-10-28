import Foundation

protocol DictationRunning: Sendable {
    func run(samples: [Float]) async -> Result<DictationResult, PennelError>
    func cancel()
}

extension DictationRunning {
    func cancel() {}
}

/// Sends one finished recording through the core's dictation pipeline.
struct DictationRunner: DictationRunning {
    static let sampleRate: UInt32 = 16_000

    private let engine: PennelEngine
    private let keys: @Sendable () -> ApiKeys
    private let active = ActiveDictation()

    init(engine: PennelEngine, keys: @escaping @Sendable () -> ApiKeys = { Keychain.apiKeys() }) {
        self.engine = engine
        self.keys = keys
    }

    /// Runs off the main actor: the core blocks on network and on-device models.
    func run(samples: [Float]) async -> Result<DictationResult, PennelError> {
        let engine = engine
        let request = DictationRequest(samples: samples, sampleRate: Self.sampleRate, appBundleId: nil, keys: keys())
        let session = engine.beginDictationSession(observer: DiscardedPreview(), practice: false)
        active.set(session)
        let active = active
        return await Task.detached(priority: .userInitiated) {
            defer { active.clear(session) }
            do {
                return .success(try session.finish(request: request))
            } catch let error as PennelError {
                return .failure(error)
            } catch {
                return .failure(.InvalidResponse(message: error.localizedDescription))
            }
        }.value
    }

    func cancel() { active.cancel() }
}

private final class DiscardedPreview: DictationPreviewObserver, @unchecked Sendable {
    func changed(update: DictationPreviewUpdate) {}
}

private final class ActiveDictation: @unchecked Sendable {
    private let lock = NSLock()
    private var current: LiveDictationSession?
    private var cancelled = false

    func set(_ session: LiveDictationSession) {
        lock.withLock {
            current = session
            if cancelled { session.cancel() }
        }
    }

    func clear(_ session: LiveDictationSession) {
        lock.withLock {
            if current === session { current = nil }
        }
    }

    func cancel() {
        lock.withLock {
            cancelled = true
            current?.cancel()
        }
    }
}
