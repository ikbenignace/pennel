import ActivityKit
import Foundation
import os
import PennelMobileKit

@MainActor
protocol SessionActivityControlling: AnyObject {
    func start(_ state: SessionActivityState)
    func update(_ state: SessionActivityState)
    func end(message: String?)
}

/// The Live Activity (Lock Screen and Dynamic Island) that shows the session is ready, with a Stop button.
@MainActor
final class SystemSessionActivity: SessionActivityControlling {
    private var activity: Activity<SessionActivityAttributes>?
    private static let log = Logger(subsystem: "app.pennel.ios", category: "session")
    /// Long enough to read why the session stopped, short enough not to linger on the Lock Screen.
    private static let messageLifetime: TimeInterval = 10 * 60

    func start(_ state: SessionActivityState) {
        guard activity == nil, ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        do {
            activity = try Activity.request(
                attributes: SessionActivityAttributes(), content: Self.content(state), pushType: nil)
        } catch {
            Self.log.error("live activity: \(ErrorText.code(error), privacy: .public)")
        }
    }

    func update(_ state: SessionActivityState) {
        guard let id = activity?.id else { return }
        let content = Self.content(state)
        Task.detached { await Self.find(id)?.update(content) }
    }

    func end(message: String?) {
        guard let id = activity?.id else { return }
        activity = nil
        let now = Date()
        let final = message.map {
            Self.content(SessionActivityState(phase: .off, expiresAt: now, level: 0, message: $0))
        }
        let policy: ActivityUIDismissalPolicy = message == nil ? .immediate : .after(now.addingTimeInterval(Self.messageLifetime))
        Task.detached { await Self.find(id)?.end(final, dismissalPolicy: policy) }
    }

    /// `Activity` is not `Sendable`, so background tasks look it up by id instead of capturing it.
    private nonisolated static func find(_ id: String) -> Activity<SessionActivityAttributes>? {
        Activity<SessionActivityAttributes>.activities.first { $0.id == id }
    }

    /// A Live Activity left by a previous run (the app was killed) must not claim the microphone is ready.
    static func endLeftovers() {
        Task.detached {
            for activity in Activity<SessionActivityAttributes>.activities {
                await activity.end(nil, dismissalPolicy: .immediate)
            }
        }
    }

    private static func content(_ state: SessionActivityState) -> ActivityContent<SessionActivityState> {
        ActivityContent(state: state, staleDate: state.phase == .ready ? state.expiresAt : nil)
    }
}
