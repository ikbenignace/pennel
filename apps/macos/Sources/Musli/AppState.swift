import Foundation
@preconcurrency import MusliCore
import Observation

@MainActor
@Observable
final class AppState {
    let engine: MusliEngine
    var settings: Settings
    var lastError: String?
    var onHotkeyChange: ((HotkeyTrigger) -> Void)?

    init() throws {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        engine = try MusliEngine(dataDir: support.appendingPathComponent("Musli").path)
        settings = engine.settings()
    }

    func save() {
        do {
            try engine.updateSettings(settings: settings)
            lastError = nil
        } catch {
            lastError = "Instellingen bewaren mislukt: \(error)"
        }
    }

    func keys() -> ApiKeys {
        ApiKeys(stt: Keychain.get(settings.dictationStt.keychainAccount), llm: Keychain.get(settings.llm.keychainAccount))
    }

    var needsOnboarding: Bool {
        Permissions.microphone != .authorized
            || !Permissions.accessibilityTrusted
            || (!settings.dictationStt.keychainAccount.isEmpty && Keychain.get(settings.dictationStt.keychainAccount) == nil)
    }
}
