import Foundation
import Testing
@testable import PennelMobileKit

struct KeyboardRefinementTests {
    @Test func accented_letters_follow_shift_and_never_replace_symbol_keys() {
        var state = KeyboardTypingState()
        #expect(state.alternatives(for: "e") == ["è", "é", "ê", "ë"])
        #expect(state.alternatives(for: "a").contains("ä"))
        #expect(state.alternatives(for: "c") == ["ç"])
        state.pressShift(at: Date())
        #expect(state.alternatives(for: "e") == ["È", "É", "Ê", "Ë"])
        state.layer = .numbers
        #expect(state.alternatives(for: "e").isEmpty)
        #expect(state.alternatives(for: "1").isEmpty)
    }

    @Test func retained_text_preview_is_bounded_and_does_not_change_the_result() {
        let text = "A sentence\nwith a second line " + String(repeating: "é", count: 200)
        let preview = KeyboardResultPreview.text(text)
        #expect(!preview.contains("\n"))
        #expect(preview.count == 161)
        #expect(preview.hasSuffix("…"))
        #expect(text.contains("\n"))
    }

    @Test func provider_setup_and_record_again_are_distinct_recovery_actions() {
        let copy = KeyboardCopy(language: .english)
        #expect(copy.recoveryAction(code: "network", message: nil) == .recordAgain)
        #expect(copy.recoveryAction(code: "provider_auth", message: nil) == .checkSettings)
        #expect(copy.recoveryAction(code: nil, message: "API-sleutel geweigerd") == .checkSettings)
        #expect(copy.text(.manualStart).contains("Dictation"))
        #expect(KeyboardCopy(language: .dutch).text(.manualStart).contains("Dicteren"))
    }
}

@MainActor
struct KeyboardFeedbackTests {
    let h = Harness()

    @Test func action_labels_follow_the_real_action() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        #expect(model.primaryActionLabel == "Tap or hold to speak")
        model.tapPill()
        #expect(model.primaryActionLabel == "Stop dictation")
        model.tapPill()
        #expect(model.primaryActionLabel == "Pennel is processing your dictation.")
        #expect(!model.primaryActionEnabled)
    }

    @Test func feedback_is_acknowledged_once_and_never_replayed_by_refresh() throws {
        try h.session(.ready)
        var events: [FeedbackEvent] = []
        let model = KeyboardSessionModel(hasFullAccess: true, client: HandoffClient(store: h.store), now: { h.now },
            context: { .init(document: h.document, lifetime: h.document, generation: h.generation) },
            onFeedback: { events.append($0) }, onResult: { h.inserted.append($0) })
        model.refresh()
        model.tapPill()
        #expect(events.isEmpty)
        try h.session(.listening)
        model.refresh(); model.refresh()
        #expect(events == [.recordingStarted])
        model.tapPill()
        try h.session(.processing)
        model.refresh(); model.refresh()
        #expect(events == [.recordingStarted, .recordingStopped])
        try h.answer("A result", to: #require(h.lastCommand).id)
        model.refresh(); model.refresh()
        #expect(events == [.recordingStarted, .recordingStopped, .resultDelivered])
        #expect(h.inserted == ["A result"])
    }

    @Test func record_again_starts_a_new_capture_instead_of_claiming_to_retry_audio() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh(); model.tapPill(); model.tapPill()
        let old = try #require(h.lastCommand)
        try h.store.write(HandoffResult(id: old.id, text: nil, error: "No speech recognized. Try again.", errorCode: "no_speech", at: h.now), as: .result)
        model.refresh()
        model.recordAgain()
        #expect(h.lastCommand?.action == .start)
        #expect(h.lastCommand?.id != old.id)
        #expect(model.pad == .listening)
    }

    @Test func an_unacknowledged_start_and_stop_do_not_claim_tactile_capture_success() throws {
        try h.session(.ready)
        var events: [FeedbackEvent] = []
        let model = KeyboardSessionModel(hasFullAccess: true, client: HandoffClient(store: h.store), now: { h.now },
            context: { .init(document: h.document, lifetime: h.document, generation: h.generation) },
            onFeedback: { events.append($0) }, onResult: { _ in })
        model.refresh(); model.tapPill(); model.tapPill(); model.refresh()
        #expect(events.isEmpty)
        h.now = h.now.addingTimeInterval(4)
        try h.session(.ready)
        model.refresh()
        #expect(events.isEmpty)
    }

    @Test func processing_cancellation_is_acknowledged_once_without_late_success_feedback() throws {
        try h.session(.ready)
        var events: [FeedbackEvent] = []
        let model = KeyboardSessionModel(hasFullAccess: true, client: HandoffClient(store: h.store), now: { h.now },
            context: { .init(document: h.document, lifetime: h.document, generation: h.generation) },
            onFeedback: { events.append($0) }, onResult: { h.inserted.append($0) })
        model.refresh(); model.tapPill()
        try h.session(.listening); model.refresh(); model.tapPill()
        let stopped = try #require(h.lastCommand)
        try h.session(.processing); model.refresh(); model.cancel()
        #expect(events == [.recordingStarted, .recordingStopped])
        try h.session(.ready); model.refresh(); model.refresh()
        try h.answer("A cancelled result", to: stopped.id); model.refresh()
        #expect(events == [.recordingStarted, .recordingStopped, .recordingCancelled])
        #expect(h.inserted.isEmpty)
    }

    @Test func check_settings_recovery_does_not_start_another_capture() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh(); model.tapPill(); model.tapPill()
        let stopped = try #require(h.lastCommand)
        try h.store.write(HandoffResult(id: stopped.id, text: nil, error: nil, errorCode: "provider_auth", at: h.now), as: .result)
        model.refresh(); model.recordAgain()
        #expect(model.recoveryAction == .checkSettings)
        #expect(h.lastCommand?.id == stopped.id)
        model.retry()
        #expect(model.pad == .ready)
        #expect(h.lastCommand?.id == stopped.id)
    }
}
