import Foundation

/// A denied "System Audio Recording" permission does not fail the tap; it just delivers zeros.
/// Flags (once) when nothing but digital silence arrived for `threshold` seconds.
public struct SilenceWatch: Sendable {
    private let threshold: TimeInterval
    private var reported = false
    public private(set) var heardSound = false

    public init(threshold: TimeInterval = 20) {
        self.threshold = threshold
    }

    /// Exact zeros only: a quiet room always carries some noise, so this means a broken capture path.
    public static func isDigitalSilence(_ samples: [Float]) -> Bool {
        !samples.isEmpty && !samples.contains(where: { $0 != 0 })
    }

    public mutating func feed(_ samples: [Float], at elapsed: TimeInterval) -> Bool {
        if !heardSound, samples.contains(where: { $0 != 0 }) { heardSound = true }
        guard !heardSound, !reported, elapsed >= threshold else { return false }
        reported = true
        return true
    }
}
