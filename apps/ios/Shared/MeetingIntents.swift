import AppIntents

struct StartMeetingIntent: AppIntent {
    static let title: LocalizedStringResource = "Start opname"
    static let supportedModes: IntentModes = .foreground

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        IntentEnvironment.router().selectedTab = .meetings
        try await IntentEnvironment.meeting().start(title: "")
        #endif
        return .result()
    }
}

/// Bound to the Stop opname button of the recording Live Activity; background-capable like the session stop.
struct StopMeetingIntent: AudioRecordingIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop opname"
    static let supportedModes: IntentModes = .background

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        await IntentEnvironment.meeting().stop()
        #endif
        return .result()
    }
}
