#if os(iOS)
import ActivityKit
import Foundation

public struct SessionActivityAttributes: ActivityAttributes {
    public typealias ContentState = SessionActivityState
    public init() {}
}

public struct SessionActivityState: Codable, Hashable, Sendable {
    public var phase: SessionPhase
    public var expiresAt: Date
    public var level: Float
    public var message: String?

    public init(phase: SessionPhase, expiresAt: Date, level: Float, message: String?) {
        self.phase = phase
        self.expiresAt = expiresAt
        self.level = level
        self.message = message
    }
}

public struct RecordingActivityAttributes: ActivityAttributes {
    public typealias ContentState = RecordingActivityState
    public init() {}
}

public struct RecordingActivityState: Codable, Hashable, Sendable {
    public enum Phase: String, Codable, Sendable { case recording, interrupted, finishing }
    public var phase: Phase
    public var startedAt: Date
    public var message: String?

    public init(phase: Phase, startedAt: Date, message: String?) {
        self.phase = phase
        self.startedAt = startedAt
        self.message = message
    }
}
#endif
