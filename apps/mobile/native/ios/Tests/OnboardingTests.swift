import Foundation
import PennelMobileKit
import Testing
@testable internal import PennelMobile

private func tempEngine() throws -> PennelEngine {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("pennel-test-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return try PennelEngine.newPreview(dataDir: dir.path)
}

private func scratchDefaults() -> UserDefaults {
    let name = "pennel-onboarding-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: name)!
    defaults.removePersistentDomain(forName: name)
    return defaults
}

struct OnboardingTests {
    @Test func onboarding_skips_cloud_when_apple_speech_chosen() throws {
        var flow = OnboardingFlow(speech: .apple)
        #expect(!flow.steps.contains(.cloud))
        #expect(flow.step(after: .speech) == .keyboard)

        flow.speech = .cloud
        #expect(flow.step(after: .speech) == .cloud)
        #expect(flow.step(after: .cloud) == .keyboard)
        #expect(flow.step(before: .keyboard) == .cloud)

        let engine = try tempEngine()
        var settings = engine.settings()
        OnboardingFlow.apply(.apple, to: &settings, cloudSignedIn: false, hasKey: { _ in false })
        let speech = try #require(settings.providers.first { $0.id == settings.dictationStt.providerId })
        #expect(speech.kind == .appleSpeech)
        #expect(settings.meetingStt.providerId == speech.id)
        #expect(!settings.providers.contains { $0.kind == .pennelCloud })
    }

    @Test func onboarding_keyboard_step_detects_full_access_state() {
        let defaults = scratchDefaults()
        #expect(KeyboardSetup.state(in: defaults) == .needsFullAccess)

        KeyboardFullAccess.markSeen(in: defaults)
        #expect(KeyboardSetup.state(in: defaults) == .fullAccess)
        #expect(defaults.dictionaryRepresentation()[KeyboardFullAccess.defaultsKey] as? Bool == true)
    }

    @Test func cloud_choice_applies_only_when_signed_in() throws {
        let engine = try tempEngine()
        var settings = engine.settings()
        let before = settings
        OnboardingFlow.apply(.cloud, to: &settings, cloudSignedIn: false, hasKey: { _ in false })
        #expect(settings == before)

        OnboardingFlow.apply(.cloud, to: &settings, cloudSignedIn: true, hasKey: { _ in false })
        let speech = try #require(settings.providers.first { $0.id == settings.dictationStt.providerId })
        #expect(speech.kind == .pennelCloud)
        #expect(settings.meetingStt.providerId == speech.id)
    }

    @Test func provider_choice_uses_a_keyed_provider_for_speech() throws {
        let engine = try tempEngine()
        var settings = engine.settings()
        let openAi = newAccount(kind: .openAi, existing: settings.providers)
        settings.providers.append(openAi)
        let before = settings
        OnboardingFlow.apply(.provider, to: &settings, cloudSignedIn: false, hasKey: { _ in false })
        #expect(settings == before)

        OnboardingFlow.apply(.provider, to: &settings, cloudSignedIn: false, hasKey: { $0 == openAi.id })
        #expect(settings.dictationStt.providerId == openAi.id)
        #expect(settings.meetingStt.providerId == openAi.id)
        #expect(settings.dictationStt.model == defaultModel(kind: .openAi, task: .dictationStt))
    }

    @Test func initial_choice_follows_the_current_speech_provider() throws {
        let engine = try tempEngine()
        var settings = engine.settings()
        OnboardingFlow.apply(.apple, to: &settings, cloudSignedIn: false, hasKey: { _ in false })
        #expect(OnboardingFlow.initialChoice(settings: settings, cloudSignedIn: true) == .apple)

        OnboardingFlow.apply(.cloud, to: &settings, cloudSignedIn: true, hasKey: { _ in false })
        #expect(OnboardingFlow.initialChoice(settings: settings, cloudSignedIn: true) == .cloud)

        settings.dictationStt = ModelChoice(providerId: "", model: "")
        #expect(OnboardingFlow.initialChoice(settings: settings, cloudSignedIn: false) == .apple)
        #expect(OnboardingFlow.initialChoice(settings: settings, cloudSignedIn: true) == .cloud)
    }
}
