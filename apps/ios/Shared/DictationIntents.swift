import AppIntents

// The start intents open the app: iOS only lets an app begin recording while it is in the foreground. They live in
// `Shared/` so the Control Center buttons in the widget extension can name them; the action itself runs in the app
// process only (`PENNEL_APP`).

struct DictateToClipboardIntent: AppIntent {
    static let title: LocalizedStringResource = "Dicteer naar klembord"
    static let supportedModes: IntentModes = .foreground

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        IntentEnvironment.router().requestDictation()
        #endif
        return .result()
    }
}

struct StartDictationSessionIntent: AppIntent {
    static let title: LocalizedStringResource = "Start dicteersessie"
    static let supportedModes: IntentModes = .foreground

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        IntentEnvironment.router().showsSessionReturn = true
        await IntentEnvironment.dictationSession().start()
        #endif
        return .result()
    }
}

/// Bound to the Stop button of the session Live Activity. It must work with the phone locked, so it stays in the
/// background; as an `AudioRecordingIntent` it is allowed to end the microphone use from there.
struct StopDictationSessionIntent: AudioRecordingIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop dicteersessie"
    static let supportedModes: IntentModes = .background

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        IntentEnvironment.dictationSession().stop()
        #endif
        return .result()
    }
}
