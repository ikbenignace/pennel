import AppIntents

/// Bound to the Stop opname button of the recording Live Activity. Like `StopDictationSessionIntent`, it performs
/// in the app process only (`PENNEL_APP`); the widget extension only needs the type.
struct StopRecordingIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop opname"
    static let isDiscoverable = false

    init() {}

    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        await MeetingRuntime.shared.stop()
        #endif
        return .result()
    }
}
