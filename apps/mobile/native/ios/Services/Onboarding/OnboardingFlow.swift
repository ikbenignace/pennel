import Foundation
import PennelMobileKit

enum OnboardingPage: Hashable {
    case welcome, microphone, speech, cloud, provider, keyboard, actionButton
}

/// The three speech choices of spec §4.7.
enum SpeechChoice: CaseIterable, Hashable {
    case cloud, apple, provider

    var title: String {
        switch self {
        case .cloud: Copy.Onboarding.pennelCloud
        case .apple: Copy.Onboarding.appleDictation
        case .provider: Copy.Onboarding.ownProvider
        }
    }

    var detail: String {
        switch self {
        case .cloud: Copy.Onboarding.pennelCloudDetail
        case .apple: Copy.Onboarding.appleDictationDetail
        case .provider: Copy.Onboarding.ownProviderDetail
        }
    }
}

enum KeyboardSetup: Equatable {
    case needsFullAccess, fullAccess

    static func state(in defaults: UserDefaults?) -> KeyboardSetup {
        KeyboardFullAccess.seen(in: defaults) ? .fullAccess : .needsFullAccess
    }
}

/// Which steps the first launch shows: Cloud login or provider setup only follow the matching speech choice.
struct OnboardingFlow: Equatable {
    var speech: SpeechChoice

    var steps: [OnboardingPage] {
        let setup: [OnboardingPage] = switch speech {
        case .cloud: [.cloud]
        case .provider: [.provider]
        case .apple: []
        }
        return [.welcome, .microphone, .speech] + setup + [.keyboard, .actionButton]
    }

    func step(after step: OnboardingPage) -> OnboardingPage? {
        guard let index = steps.firstIndex(of: step), index + 1 < steps.count else { return nil }
        return steps[index + 1]
    }

    func step(before step: OnboardingPage) -> OnboardingPage? {
        guard let index = steps.firstIndex(of: step), index > 0 else { return nil }
        return steps[index - 1]
    }

    /// Preselects what speech already uses, so reopening the introduction does not suggest a change.
    static func initialChoice(settings: Settings, cloudSignedIn: Bool) -> SpeechChoice {
        let kind = settings.providers.first { $0.id == settings.dictationStt.providerId }?.kind
        switch kind {
        case .pennelCloud: return .cloud
        case .appleSpeech: return .apple
        case .some(let kind) where isOwnProvider(kind): return .provider
        default: return cloudSignedIn ? .cloud : .apple
        }
    }

    /// Points dictation and meeting speech at the choice. Cloud needs a login and an own provider needs a usable
    /// account first; until then the current choice stays.
    static func apply(
        _ choice: SpeechChoice, to settings: inout Settings, cloudSignedIn: Bool, hasKey: (String) -> Bool
    ) {
        let account: ProviderAccount
        switch choice {
        case .apple:
            account = FirstRunDefaults.account(.appleSpeech, in: &settings)
        case .cloud:
            guard cloudSignedIn else { return }
            account = FirstRunDefaults.account(.pennelCloud, in: &settings)
        case .provider:
            guard let usable = settings.providers.first(where: {
                isOwnProvider($0.kind) && supports(kind: $0.kind, task: .dictationStt)
                    && (!requiresKey(kind: $0.kind) || hasKey($0.id))
            }) else { return }
            account = usable
        }
        // A slot already on this account keeps the model the user picked.
        if settings.dictationStt.providerId != account.id {
            settings.dictationStt = ModelChoice(providerId: account.id, model: defaultModel(kind: account.kind, task: .dictationStt))
        }
        if settings.meetingStt.providerId != account.id {
            settings.meetingStt = ModelChoice(providerId: account.id, model: defaultModel(kind: account.kind, task: .meetingStt))
        }
    }

    private static func isOwnProvider(_ kind: ProviderKind) -> Bool {
        ![.local, .appleSpeech, .appleIntelligence, .pennelCloud].contains(kind) && !isCliAgent(kind: kind)
    }
}
