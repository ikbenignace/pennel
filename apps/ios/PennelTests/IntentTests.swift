import AppIntents
import Foundation
import PennelMobileKit
import Testing
@testable import Pennel

@MainActor
private final class FakeMeeting: MeetingControlling {
    private(set) var started: [String] = []
    private(set) var stops = 0
    var failure: Error?

    func start(title: String) async throws {
        if let failure { throw failure }
        started.append(title)
    }

    func stop() async { stops += 1 }
}

@MainActor
private final class FakeSession: DictationSessionControlling {
    private(set) var starts = 0
    private(set) var stops = 0

    func start() async -> Bool {
        starts += 1
        return true
    }

    func stop() { stops += 1 }
}

@MainActor
struct IntentTests {
    /// A Live Activity button must work with the phone locked: a foreground stop would open the app.
    @Test func stop_intents_do_not_require_foreground() {
        #expect(!StopDictationSessionIntent.supportedModes.contains(.foreground))
        #expect(StopDictationSessionIntent.supportedModes.contains(.background))
        #expect(!StopMeetingIntent.supportedModes.contains(.foreground))
        #expect(StopMeetingIntent.supportedModes.contains(.background))
        #expect(StopDictationSessionIntent() is any AudioRecordingIntent)
        #expect(StopDictationSessionIntent() is any LiveActivityIntent)
        #expect(StopMeetingIntent() is any AudioRecordingIntent)
        #expect(StopMeetingIntent() is any LiveActivityIntent)
    }

    @Test func start_intents_open_the_app() {
        #expect(StartMeetingIntent.supportedModes.contains(.foreground))
        #expect(StartDictationSessionIntent.supportedModes.contains(.foreground))
        #expect(DictateToClipboardIntent.supportedModes.contains(.foreground))
    }

    @Test func start_meeting_intent_starts_runtime() async throws {
        let fake = FakeMeeting()
        IntentEnvironment.meeting = { fake }
        defer { IntentEnvironment.reset() }

        _ = try await StartMeetingIntent().perform()

        #expect(fake.started == [""])
        #expect(fake.stops == 0)
    }

    @Test func start_meeting_intent_reports_a_failed_start() async {
        let fake = FakeMeeting()
        fake.failure = MeetingRuntime.StartError.busy
        IntentEnvironment.meeting = { fake }
        defer { IntentEnvironment.reset() }

        await #expect(throws: MeetingRuntime.StartError.self) { _ = try await StartMeetingIntent().perform() }
    }

    @Test func stop_intents_stop_what_they_control() async throws {
        let meeting = FakeMeeting()
        let session = FakeSession()
        IntentEnvironment.meeting = { meeting }
        IntentEnvironment.dictationSession = { session }
        defer { IntentEnvironment.reset() }

        _ = try await StopMeetingIntent().perform()
        _ = try await StopDictationSessionIntent().perform()

        #expect(meeting.stops == 1)
        #expect(session.stops == 1)
    }

    @Test func start_session_intent_starts_session_and_shows_return_screen() async throws {
        let session = FakeSession()
        IntentEnvironment.dictationSession = { session }
        let router = AppRouter()
        IntentEnvironment.router = { router }
        defer { IntentEnvironment.reset() }

        _ = try await StartDictationSessionIntent().perform()

        #expect(session.starts == 1)
        #expect(router.showsSessionReturn)
    }

    @Test func dictate_intent_opens_dictation_tab_and_requests_a_dictation() async throws {
        let router = AppRouter()
        IntentEnvironment.router = { router }
        defer { IntentEnvironment.reset() }

        _ = try await DictateToClipboardIntent().perform()

        #expect(router.selectedTab == .dictate)
        #expect(router.dictationRequests == 1)
    }

    @Test func intent_titles_match_the_spec() {
        #expect(String(localized: DictateToClipboardIntent.title) == Copy.Intents.dictateToClipboard)
        #expect(String(localized: StartDictationSessionIntent.title) == Copy.Intents.startDictationSession)
        #expect(String(localized: StopDictationSessionIntent.title) == Copy.Intents.stopDictationSession)
        #expect(String(localized: StartMeetingIntent.title) == Copy.Intents.startRecording)
        #expect(String(localized: StopMeetingIntent.title) == Copy.Intents.stopRecording)
    }

    /// Phrases are extracted from the `AppShortcutsProvider` at build time, so the built metadata is the real source.
    @Test func shortcut_phrases_are_dutch_and_include_app_name() throws {
        let url = try #require(Bundle.main.url(forResource: "extract", withExtension: "actionsdata", subdirectory: "Metadata.appintents"))
        let json = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let shortcuts = try #require(json["autoShortcuts"] as? [[String: Any]])
        var phrases: [String: [String]] = [:]
        for shortcut in shortcuts {
            let action = try #require(shortcut["actionIdentifier"] as? String)
            let templates = try #require(shortcut["phraseTemplates"] as? [[String: Any]])
            phrases[action] = templates.compactMap { $0["key"] as? String }
        }
        #expect(phrases == [
            "DictateToClipboardIntent": ["Dicteer met ${applicationName}", "Dicteer naar klembord met ${applicationName}"],
            "StartDictationSessionIntent": ["Start een dicteersessie met ${applicationName}", "Start dicteersessie in ${applicationName}"],
            "StopDictationSessionIntent": ["Stop de dicteersessie van ${applicationName}"],
            "StartMeetingIntent": ["Start een opname met ${applicationName}", "Start opname in ${applicationName}"],
            "StopMeetingIntent": ["Stop de opname van ${applicationName}"],
        ])
        #expect(phrases.values.joined().allSatisfy { $0.contains("${applicationName}") })
    }

    @Test func session_duration_defaults_to_five_minutes_and_round_trips() throws {
        let defaults = try #require(UserDefaults(suiteName: "intent-tests-\(UUID().uuidString)"))
        #expect(SessionClock.Duration.stored(in: defaults) == .fiveMinutes)
        SessionClock.Duration.oneHour.store(in: defaults)
        #expect(SessionClock.Duration.stored(in: defaults) == .oneHour)
        defaults.set(42.0, forKey: SessionClock.Duration.storageKey)
        #expect(SessionClock.Duration.stored(in: defaults) == .fiveMinutes)
    }
}
