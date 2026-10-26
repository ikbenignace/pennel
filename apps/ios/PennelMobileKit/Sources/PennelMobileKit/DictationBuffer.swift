import Foundation

/// Samples are only kept while a dictation is listening; anything captured in `ready` is dropped on arrival.
public final class DictationBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [Float] = []

    public init() {}

    public func append(_ new: [Float], phase: SessionPhase) {
        guard phase == .listening else { return }
        lock.withLock { samples.append(contentsOf: new) }
    }

    public func take() -> [Float] {
        lock.withLock {
            defer { samples = [] }
            return samples
        }
    }
}
