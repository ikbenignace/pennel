import ActivityKit
import Foundation

/// Compiled into the app (which starts and updates the Live Activity) and the widget extension (which draws it).
struct RecordingActivityAttributes: ActivityAttributes {
    typealias ContentState = RecordingActivityState
}

struct RecordingActivityState: Codable, Hashable, Sendable {
    enum Phase: String, Codable, Sendable { case recording, interrupted, finishing }

    var phase: Phase
    /// The clock counts from here, so the system updates the time without the app.
    var startedAt: Date
    var message: String?
}
