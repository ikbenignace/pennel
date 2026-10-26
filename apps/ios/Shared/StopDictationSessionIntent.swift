import AppIntents

/// Bound to the Stop button of the session Live Activity. A `LiveActivityIntent` always performs in the app
/// process, so only the app build (`PENNEL_APP`) carries the action; the widget extension only needs the type.
struct StopDictationSessionIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop dicteersessie"
    static let isDiscoverable = false

    init() {}

    func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        await DictationSession.shared.stop()
        #endif
        return .result()
    }
}
