import Foundation

@MainActor
protocol MeetingControlling: AnyObject {
    func start(title: String) async throws
    func stop() async
}

@MainActor
protocol DictationSessionControlling: AnyObject {
    @discardableResult func start() async -> Bool
    func stop()
}

/// Asks for the microphone first, like the Nieuwe opname button: an intent has no screen to show the system prompt.
@MainActor
final class LiveMeetingControl: MeetingControlling {
    func start(title: String) async throws {
        guard await MicCapture.requestPermission() else { throw MeetingRuntime.StartError.microphone }
        try await MeetingRuntime.shared.start(title: title)
    }

    func stop() async { await MeetingRuntime.shared.stop() }
}

extension DictationSession: DictationSessionControlling {}

/// What the intents act on. Tests swap these for fakes.
@MainActor
enum IntentEnvironment {
    static var meeting: () -> MeetingControlling = { LiveMeetingControl() }
    static var dictationSession: () -> DictationSessionControlling = { DictationSession.shared }
    static var router: () -> AppRouter = { AppRouter.shared }

    static func reset() {
        meeting = { LiveMeetingControl() }
        dictationSession = { DictationSession.shared }
        router = { AppRouter.shared }
    }
}
