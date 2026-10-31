import Foundation
import Testing
@testable import PennelMobileKit

struct FeedbackTests {
    @Test func defaults_enable_action_feedback_but_leave_typing_optional() {
        #expect(KeyboardPreferences.feedbackEnabled(in: nil))
        #expect(!KeyboardPreferences.typingFeedbackEnabled(in: nil))
        #expect(FeedbackPolicy.pattern(for: .recordingStarted, feedbackEnabled: true, typingEnabled: false) == .lightImpact)
        #expect(FeedbackPolicy.pattern(for: .keyPress, feedbackEnabled: true, typingEnabled: false) == nil)
    }

    @Test func typing_feedback_and_action_feedback_have_independent_preferences() {
        #expect(FeedbackPolicy.pattern(for: .keyPress, feedbackEnabled: false, typingEnabled: true) == .softImpact)
        #expect(FeedbackPolicy.pattern(for: .resultDelivered, feedbackEnabled: false, typingEnabled: true) == nil)
        #expect(FeedbackPolicy.pattern(for: .deviceConnected, feedbackEnabled: true, typingEnabled: false) == .success)
        #expect(FeedbackPolicy.pattern(for: .error, feedbackEnabled: true, typingEnabled: false) == .error)
    }

    @Test func feedback_choices_can_be_mirrored_without_full_access() {
        let sharedName = "pennel-feedback-shared-\(UUID().uuidString)"
        let localName = "pennel-feedback-local-\(UUID().uuidString)"
        let shared = UserDefaults(suiteName: sharedName)!
        let local = UserDefaults(suiteName: localName)!
        defer { shared.removePersistentDomain(forName: sharedName); local.removePersistentDomain(forName: localName) }
        KeyboardPreferences.setFeedbackEnabled(false, in: shared)
        KeyboardPreferences.setTypingFeedbackEnabled(true, in: shared)
        KeyboardPreferences.mirrorFeedback(from: shared, to: local)
        #expect(!KeyboardPreferences.feedbackEnabled(in: local))
        #expect(KeyboardPreferences.typingFeedbackEnabled(in: local))
    }
}
