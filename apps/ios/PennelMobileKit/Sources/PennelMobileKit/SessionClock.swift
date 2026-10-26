import Foundation

public struct SessionClock: Equatable, Sendable {
    public enum Duration: TimeInterval, CaseIterable, Sendable {
        case fiveMinutes = 300
        case fifteenMinutes = 900
        case oneHour = 3600
    }

    public let duration: TimeInterval
    public private(set) var expiresAt: Date

    public init(duration: Duration, now: Date) {
        self.init(duration: duration.rawValue, now: now)
    }

    public init(duration: TimeInterval, now: Date) {
        self.duration = duration
        expiresAt = now.addingTimeInterval(duration)
    }

    public mutating func extend(now: Date) {
        expiresAt = now.addingTimeInterval(duration)
    }

    public func isExpired(now: Date) -> Bool {
        now >= expiresAt
    }

    public func remaining(now: Date) -> TimeInterval {
        max(0, expiresAt.timeIntervalSince(now))
    }
}
