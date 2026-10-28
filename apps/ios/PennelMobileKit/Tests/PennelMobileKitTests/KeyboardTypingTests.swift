import Foundation
import Testing
@testable import PennelMobileKit

struct KeyboardTypingTests {
    let now = Date(timeIntervalSince1970: 1_800_000_000)

    @Test func shift_is_one_shot_and_double_tap_locks() {
        var state = KeyboardTypingState()
        state.pressShift(at: now)
        #expect(state.shift == .once)
        state.insertedCharacter()
        #expect(state.shift == .off)
        state.pressShift(at: now.addingTimeInterval(1))
        state.pressShift(at: now.addingTimeInterval(1.2))
        #expect(state.shift == .locked)
        state.insertedCharacter()
        #expect(state.shift == .locked)
        state.pressShift(at: now.addingTimeInterval(2))
        #expect(state.shift == .off)
    }

    @Test func capitalization_obeys_context_and_manual_shift() {
        var state = KeyboardTypingState()
        state.updateCapitalization(before: "Hello. ", mode: .sentences)
        #expect(state.shift == .once)
        state.updateCapitalization(before: "Hello w", mode: .sentences)
        #expect(state.shift == .off)
        state.pressShift(at: now)
        state.updateCapitalization(before: "Hello w", mode: .sentences)
        #expect(state.shift == .once)
        state.insertedCharacter()
        state.updateCapitalization(before: "Hello World ", mode: .words)
        #expect(state.shift == .once)
        state.updateCapitalization(before: nil, mode: .none)
        #expect(state.shift == .off)
    }

    @Test func double_tap_caps_lock_also_works_from_automatic_sentence_shift() {
        var state = KeyboardTypingState()
        state.updateCapitalization(before: "", mode: .sentences)
        #expect(state.shift == .once)
        state.pressShift(at: now)
        #expect(state.shift == .off)
        state.pressShift(at: now.addingTimeInterval(0.2))
        #expect(state.shift == .locked)
    }

    @Test func all_layouts_have_distinct_number_and_symbol_layers() {
        var state = KeyboardTypingState()
        #expect(state.rows(layout: .azerty)[0].first == "a")
        state.layer = .numbers
        #expect(state.rows(layout: .qwerty)[0] == Array("1234567890").map(String.init))
        state.layer = .symbols
        #expect(state.rows(layout: .azerty)[0].contains("["))
        #expect(!state.rows(layout: .azerty)[0].contains("1"))
    }

    @Test func double_space_replaces_only_a_recent_space_after_a_word() {
        var state = KeyboardTypingState()
        #expect(state.spaceEdit(before: "Hello", at: now) == .insert(" "))
        #expect(state.spaceEdit(before: "Hello ", at: now.addingTimeInterval(0.3)) == .replacePreviousCharacter(". "))
        #expect(state.spaceEdit(before: "Hello. ", at: now.addingTimeInterval(0.4)) == .insert(" "))
        state.resetSpacing()
        _ = state.spaceEdit(before: "Hello", at: now)
        #expect(state.spaceEdit(before: "Hello ", at: now.addingTimeInterval(2)) == .insert(" "))
        state.resetSpacing()
        _ = state.spaceEdit(before: "user@example.com", at: now, allowsPeriod: false)
        #expect(state.spaceEdit(before: "user@example.com ", at: now.addingTimeInterval(0.3), allowsPeriod: false) == .insert(" "))
    }

    @Test func suggestions_are_bounded_local_and_case_aware() {
        let index = KeyboardSuggestionIndex(
            words: ["Pennel", "pen", "pencil", "Pen", "  ", String(repeating: "x", count: 90)],
            supplementary: [KeyboardLexiconEntry(input: "omw", replacement: "On my way")])
        #expect(index.suggestions(for: "pe") == ["Pen", "pencil", "Pennel"])
        #expect(index.suggestions(for: "PE").allSatisfy { $0 == $0.uppercased() })
        #expect(index.suggestions(for: "omw") == ["On my way"])
        #expect(index.suggestions(for: "p").isEmpty)
        #expect(index.suggestions(for: String(repeating: "x", count: 70)).isEmpty)
        #expect(index.suggestions(for: "pe", limit: 1).count == 1)
    }
}

struct KeyboardPreferencesTests {
    @Test func local_suggestions_require_opt_in_and_remove_dictionary_when_disabled() {
        let defaults = UserDefaults(suiteName: "pennel-keyboard-prefs-\(UUID().uuidString)")!
        #expect(!KeyboardPreferences.suggestionsEnabled(in: defaults))
        KeyboardPreferences.publish(dictionary: ["Pennel"], interfaceLanguage: "nl", in: defaults)
        #expect(KeyboardPreferences.dictionary(in: defaults).isEmpty)
        KeyboardPreferences.setSuggestionsEnabled(true, in: defaults)
        KeyboardPreferences.publish(dictionary: ["Pennel", "Pennel", "AI"], interfaceLanguage: "en", in: defaults)
        #expect(KeyboardPreferences.dictionary(in: defaults) == ["AI", "Pennel"])
        #expect(KeyboardPreferences.interfaceLanguage(in: defaults, preferredLanguages: ["nl-BE"]) == .english)
        KeyboardPreferences.setSuggestionsEnabled(false, in: defaults)
        #expect(KeyboardPreferences.dictionary(in: defaults).isEmpty)
        #expect(defaults.object(forKey: KeyboardPreferences.dictionaryKey) == nil)
    }

    @Test func language_defaults_to_first_supported_system_language() {
        #expect(KeyboardPreferences.interfaceLanguage(in: nil, preferredLanguages: ["fr-BE", "nl-BE", "en-US"]) == .dutch)
        #expect(KeyboardPreferences.interfaceLanguage(in: nil, preferredLanguages: ["fr-BE"]) == .english)
        for language in KeyboardInterfaceLanguage.allCases {
            let copy = KeyboardCopy(language: language)
            #expect(KeyboardCopy.Key.allCases.allSatisfy { !copy.text($0).isEmpty })
        }
    }

    @Test func production_and_preview_handoff_namespaces_are_separate() {
        let production = HandoffConfiguration(info: [:])
        let preview = HandoffConfiguration(info: [
            "PennelAppGroupIdentifier": "group.app.pennel.preview",
            "PennelHandoffNamespace": "app.pennel.ios.preview.handoff",
        ])
        #expect(production.appGroup == "group.app.pennel")
        #expect(production.namespace == "app.pennel.handoff")
        #expect(preview.appGroup != production.appGroup)
        #expect(HandoffSignal.result.notificationName(namespace: preview.namespace) == "app.pennel.ios.preview.handoff.result")
        #expect(HandoffSignal.result.notificationName(namespace: preview.namespace) != HandoffSignal.result.notificationName(namespace: production.namespace))
    }

    @Test func invalid_explicit_profile_configuration_never_falls_back_to_production() {
        let invalid = HandoffConfiguration(info: ["PennelAppGroupIdentifier": "", "PennelHandoffNamespace": 1])
        #expect(invalid.appGroup != "group.app.pennel")
        #expect(invalid.namespace != "app.pennel.handoff")
    }
}
