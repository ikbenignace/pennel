import AppIntents
#if PENNEL_APP
internal import PennelMobile
#endif

struct DictateToClipboardIntent: AppIntent {
    static let title: LocalizedStringResource = "Dictate to clipboard"
    static let supportedModes: IntentModes = .foreground

    @MainActor func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        try await PennelNativeRuntime.shared.startDictation()
        #endif
        return .result()
    }
}

struct StartDictationSessionIntent: AppIntent {
    static let title: LocalizedStringResource = "Start dictation session"
    static let supportedModes: IntentModes = .foreground

    @MainActor func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        try await PennelNativeRuntime.shared.startSession()
        #endif
        return .result()
    }
}

struct StopDictationSessionIntent: AudioRecordingIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop dictation session"
    static let supportedModes: IntentModes = .background

    @MainActor func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        PennelNativeRuntime.shared.stopSession()
        #endif
        return .result()
    }
}

struct StartMeetingIntent: AppIntent {
    static let title: LocalizedStringResource = "Start recording"
    static let supportedModes: IntentModes = .foreground

    @MainActor func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        try await PennelNativeRuntime.shared.startMeeting(title: "")
        #endif
        return .result()
    }
}

struct StopMeetingIntent: AudioRecordingIntent, LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop recording"
    static let supportedModes: IntentModes = .background

    @MainActor func perform() async throws -> some IntentResult {
        #if PENNEL_APP
        await PennelNativeRuntime.shared.stopMeeting()
        #endif
        return .result()
    }
}

struct PennelShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: DictateToClipboardIntent(), phrases: ["Dictate with \(.applicationName)"], shortTitle: "Dictate", systemImageName: "mic")
        AppShortcut(intent: StartDictationSessionIntent(), phrases: ["Start a session with \(.applicationName)"], shortTitle: "Dictation session", systemImageName: "mic.circle")
        AppShortcut(intent: StartMeetingIntent(), phrases: ["Record a meeting with \(.applicationName)"], shortTitle: "Record meeting", systemImageName: "waveform")
    }
}
