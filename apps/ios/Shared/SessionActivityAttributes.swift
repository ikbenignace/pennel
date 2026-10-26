import ActivityKit
import Foundation
import PennelMobileKit

/// Compiled into the app (which starts and updates the Live Activity) and the widget extension (which draws it).
struct SessionActivityAttributes: ActivityAttributes {
    typealias ContentState = SessionActivityState
}

struct SessionActivityState: Codable, Hashable, Sendable {
    var phase: SessionPhase
    var expiresAt: Date
    var level: Float
    /// Shown instead of the countdown once the session has ended, e.g. after a phone call.
    var message: String?
}
