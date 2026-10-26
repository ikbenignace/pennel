import ActivityKit
import Foundation
import os

@MainActor
protocol RecordingActivityControlling: AnyObject {
    func start(_ state: RecordingActivityState)
    func update(_ state: RecordingActivityState)
    func end()
}

/// The Live Activity (Lock Screen and Dynamic Island) of a recording meeting: time, bars and **Stop opname**.
@MainActor
final class SystemRecordingActivity: RecordingActivityControlling {
    private var activity: Activity<RecordingActivityAttributes>?
    private static let log = Logger(subsystem: "app.pennel.ios", category: "meeting")

    func start(_ state: RecordingActivityState) {
        guard activity == nil, ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        do {
            activity = try Activity.request(
                attributes: RecordingActivityAttributes(), content: ActivityContent(state: state, staleDate: nil), pushType: nil)
        } catch {
            Self.log.error("live activity: \(ErrorText.code(error), privacy: .public)")
        }
    }

    func update(_ state: RecordingActivityState) {
        guard let id = activity?.id else { return }
        let content = ActivityContent(state: state, staleDate: nil)
        Task.detached { await Self.find(id)?.update(content) }
    }

    func end() {
        guard let id = activity?.id else { return }
        activity = nil
        Task.detached { await Self.find(id)?.end(nil, dismissalPolicy: .immediate) }
    }

    private nonisolated static func find(_ id: String) -> Activity<RecordingActivityAttributes>? {
        Activity<RecordingActivityAttributes>.activities.first { $0.id == id }
    }

    /// A recording activity left by a killed run would show a meeting that no longer records. Read synchronously at
    /// launch, before a recording can start, as for the session activity.
    static func endLeftovers() {
        let ids = Activity<RecordingActivityAttributes>.activities.map(\.id)
        SystemSessionActivity.endLeftovers(ids) { id in await find(id)?.end(nil, dismissalPolicy: .immediate) }
    }
}
