import Foundation

/// The core's fresh-install defaults point speech at the Local provider, which iPhone cannot run. A slot that is
/// unset or points at something unavailable here moves to the Apple providers; any other choice stays.
enum FirstRunDefaults {
    static func apply(
        to engine: PennelEngine,
        appleIntelligenceAvailable: Bool,
        hasKey: (String) -> Bool = { Keychain.hasKey(for: $0) }
    ) throws {
        var settings = engine.settings()
        let original = settings

        let dictationUnset = isUnset(settings.dictationStt, in: settings, unavailable: [.local])
        let meetingUnset = isUnset(settings.meetingStt, in: settings, unavailable: [.local])
        if dictationUnset || meetingUnset {
            let speech = account(.appleSpeech, in: &settings)
            let choice = ModelChoice(providerId: speech.id, model: defaultModel(kind: .appleSpeech, task: .dictationStt))
            if dictationUnset { settings.dictationStt = choice }
            if meetingUnset { settings.meetingStt = choice }
        }

        // The factory LLM is Groq without a key; until the user adds a key or picks another model it is not a choice.
        let untouchedFactoryLlm = original.llm == defaultSettings().llm && !hasKey(original.llm.providerId)
        if untouchedFactoryLlm || isUnset(original.llm, in: original, unavailable: [.local], includingCliAgents: true) {
            if appleIntelligenceAvailable {
                let ai = account(.appleIntelligence, in: &settings)
                settings.llm = ModelChoice(providerId: ai.id, model: defaultModel(kind: .appleIntelligence, task: .llm))
                settings.mode = .clean
            } else {
                settings.mode = .fast
            }
        }

        if settings != original { try engine.updateSettings(settings: settings) }
    }

    private static func isUnset(
        _ choice: ModelChoice, in settings: Settings, unavailable: [ProviderKind], includingCliAgents: Bool = false
    ) -> Bool {
        guard !choice.providerId.isEmpty, let account = settings.providers.first(where: { $0.id == choice.providerId }) else {
            return true
        }
        return unavailable.contains(account.kind) || (includingCliAgents && isCliAgent(kind: account.kind))
    }

    static func account(_ kind: ProviderKind, in settings: inout Settings) -> ProviderAccount {
        if let existing = settings.providers.first(where: { $0.kind == kind }) { return existing }
        let account = newAccount(kind: kind, existing: settings.providers)
        settings.providers.append(account)
        return account
    }
}
