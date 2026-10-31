import AppIntents
import Foundation
import PennelMobileKit
import Testing
@testable internal import Pennel
@testable internal import PennelMobile

@Suite(.serialized)
@MainActor
struct IntentTests {
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

    @Test func intent_titles_match_the_current_native_actions() {
        func english(_ resource: LocalizedStringResource) -> String {
            var resource = resource
            resource.locale = Locale(identifier: "en")
            return String(localized: resource)
        }
        #expect(english(DictateToClipboardIntent.title) == "Dictate to clipboard")
        #expect(english(StartDictationSessionIntent.title) == "Start dictation session")
        #expect(english(StopDictationSessionIntent.title) == "Stop dictation session")
        #expect(english(StartMeetingIntent.title) == "Start recording")
        #expect(english(StopMeetingIntent.title) == "Stop recording")
    }

    @Test func generated_host_extracts_current_shortcuts_with_app_name() throws {
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
            "DictateToClipboardIntent": ["Dictate with ${applicationName}"],
            "StartDictationSessionIntent": ["Start a session with ${applicationName}"],
            "StartMeetingIntent": ["Record a meeting with ${applicationName}"],
        ])
        #expect(phrases.values.joined().allSatisfy { $0.contains("${applicationName}") })
    }

    @Test func session_duration_defaults_to_five_minutes_and_round_trips() throws {
        let suite = "intent-tests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        #expect(SessionClock.Duration.stored(in: defaults) == .fiveMinutes)
        SessionClock.Duration.oneHour.store(in: defaults)
        #expect(SessionClock.Duration.stored(in: defaults) == .oneHour)
        defaults.set(42.0, forKey: SessionClock.Duration.storageKey)
        #expect(SessionClock.Duration.stored(in: defaults) == .fiveMinutes)
    }
}
