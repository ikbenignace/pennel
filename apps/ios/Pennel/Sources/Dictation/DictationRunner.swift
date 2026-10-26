import Foundation

/// Sends one finished recording through the core's dictation pipeline.
struct DictationRunner: Sendable {
    static let sampleRate: UInt32 = 16_000

    private let engine: PennelEngine
    private let keys: @Sendable () -> ApiKeys

    init(engine: PennelEngine, keys: @escaping @Sendable () -> ApiKeys = { Keychain.apiKeys() }) {
        self.engine = engine
        self.keys = keys
    }

    /// Runs off the main actor: the core blocks on network and on-device models.
    func run(samples: [Float]) async -> Result<DictationResult, PennelError> {
        let engine = engine
        let request = DictationRequest(samples: samples, sampleRate: Self.sampleRate, appBundleId: nil, keys: keys())
        return await Task.detached(priority: .userInitiated) {
            do {
                return .success(try engine.dictate(request: request))
            } catch let error as PennelError {
                return .failure(error)
            } catch {
                return .failure(.InvalidResponse(message: error.localizedDescription))
            }
        }.value
    }
}
