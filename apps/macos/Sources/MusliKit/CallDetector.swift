import Foundation

public struct MicUser: Hashable, Sendable {
    public let bundleId: String
    /// A known call app, or a browser whose window title looks like a call.
    public let isCall: Bool
    /// Calls play audio; recording a voice message does not.
    public let isPlaying: Bool

    public init(bundleId: String, isCall: Bool, isPlaying: Bool) {
        self.bundleId = bundleId
        self.isCall = isCall
        self.isPlaying = isPlaying
    }
}

public enum CallEvent: Equatable, Sendable {
    case started(String)
    case ended(String)
}

/// Turns periodic "who is using the microphone" snapshots into call start/end events.
/// Requires sustained use before starting and a long silence before ending, so a voice memo
/// or a muted minute doesn't count.
public struct CallDetector: Sendable {
    private let ownBundleId: String
    private let startAfter: TimeInterval
    private let endAfter: TimeInterval
    private var candidate: (bundleId: String, since: TimeInterval)?
    private var lastSeen: TimeInterval = 0
    public private(set) var activeCall: String?

    public init(ownBundleId: String, startAfter: TimeInterval = 5, endAfter: TimeInterval = 30) {
        self.ownBundleId = ownBundleId
        self.startAfter = startAfter
        self.endAfter = endAfter
    }

    public mutating func update(users: Set<MicUser>, at t: TimeInterval) -> [CallEvent] {
        let calls = Set(users.filter { $0.isCall && $0.isPlaying && $0.bundleId != ownBundleId }.map(\.bundleId))

        if let active = activeCall {
            // Once a call runs, any mic use by that app keeps it alive (a browser may show another tab).
            if users.contains(where: { $0.bundleId == active }) {
                lastSeen = t
                return []
            }
            guard t - lastSeen >= endAfter else { return [] }
            activeCall = nil
            candidate = nil
            return [.ended(active)]
        }

        guard let current = candidate, calls.contains(current.bundleId) else {
            candidate = calls.sorted().first.map { ($0, t) }
            return []
        }
        guard t - current.since >= startAfter else { return [] }
        activeCall = current.bundleId
        lastSeen = t
        candidate = nil
        return [.started(current.bundleId)]
    }
}
