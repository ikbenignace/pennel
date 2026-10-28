import Foundation
import Testing
@testable import PennelMobileKit

struct ActivityCopyTests {
    @Test func explicit_interface_choice_wins_over_system_language() {
        let defaults = UserDefaults(suiteName: "pennel-activity-copy-\(UUID().uuidString)")!
        KeyboardPreferences.publish(dictionary: [], interfaceLanguage: "nl", in: defaults)
        let dutch = ActivityCopy.preferences(in: defaults, preferredLanguages: ["en-US"])
        #expect(dutch.language == .dutch)
        #expect(dutch.text(.stopRecording) == "Stop opname")
        #expect(dutch.text(.dictationSession) == "Dicteersessie")
        KeyboardPreferences.publish(dictionary: [], interfaceLanguage: "en", in: defaults)
        let english = ActivityCopy.preferences(in: defaults, preferredLanguages: ["nl-BE"])
        #expect(english.language == .english)
        #expect(english.text(.stopRecording) == "Stop recording")
        #expect(english.text(.dictationSession) == "Dictation session")
    }

    @Test func first_supported_system_language_and_english_fallback_are_shared() {
        #expect(ActivityCopy.preferences(in: nil, preferredLanguages: ["fr-BE", "nl-BE", "en-US"]).language == .dutch)
        #expect(ActivityCopy.preferences(in: nil, preferredLanguages: ["fr-BE"]).language == .english)
        for language in KeyboardInterfaceLanguage.allCases {
            let copy = ActivityCopy(language: language)
            #expect(ActivityCopy.Key.allCases.allSatisfy { !copy.text($0).isEmpty })
        }
    }

    @Test func shared_labels_match_the_canonical_locales() throws {
        var root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while !FileManager.default.fileExists(atPath: root.appendingPathComponent("assets/locales/common.en.json").path), root.path != "/" {
            root.deleteLastPathComponent()
        }
        for language in KeyboardInterfaceLanguage.allCases {
            let commonURL = root.appendingPathComponent("assets/locales/common.\(language.rawValue).json")
            let meetingsURL = root.appendingPathComponent("assets/locales/meetings.\(language.rawValue).json")
            let common = try JSONDecoder().decode([String: String].self, from: Data(contentsOf: commonURL))
            let meetings = try JSONDecoder().decode([String: String].self, from: Data(contentsOf: meetingsURL))
            let copy = ActivityCopy(language: language)
            #expect(copy.text(.stop) == common["common.stop"])
            #expect(copy.text(.stopRecording) == common["common.stop_recording"])
            #expect(copy.text(.recordingActive) == meetings["meetings.recording_active"])
            #expect(copy.text(.finishingRecording) == meetings["meetings.finishing_recording"])
        }
    }

    @Test func known_native_status_messages_translate_without_rewriting_unknown_errors() {
        let en = ActivityCopy(language: .english)
        let nl = ActivityCopy(language: .dutch)
        #expect(en.sessionMessage(Copy.Session.stoppedByCall) == "Session stopped by a call.")
        #expect(nl.sessionMessage("Session stopped by a call.") == Copy.Session.stoppedByCall)
        #expect(en.sessionMessage(nil) == "Session off")
        #expect(en.recordingMessage("Opname gepauzeerd door een oproep. Pennel neemt verder op zodra de oproep voorbij is.") == "Recording paused by a call. Pennel resumes after the call ends.")
        #expect(nl.recordingMessage(nil) == "Opname gepauzeerd")
        #expect(en.recordingMessage("Audio device error 42") == "Audio device error 42")
    }

    @Test func control_configuration_resources_use_the_selected_interface_language() {
        let en = ActivityCopy(language: .english)
        let nl = ActivityCopy(language: .dutch)
        #expect(String(localized: en.resource(.dictationSession)) == "Dictation session")
        #expect(String(localized: nl.resource(.dictationSession)) == "Dicteersessie")
        #expect(String(localized: en.resource(.stopRecording)) == "Stop recording")
        #expect(String(localized: nl.resource(.stopRecording)) == "Stop opname")
    }
}
