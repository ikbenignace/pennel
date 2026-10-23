import Foundation
@preconcurrency import MusliCore
import Observation

@MainActor
@Observable
final class AppState {
    let engine: MusliEngine
    let downloads: ModelDownloads
    var settings: Settings
    var lastError: String?
    var onHotkeyChange: ((HotkeyTrigger) -> Void)?
    /// Keychain is not observable; bumping this re-renders views that show key status.
    private var keyRevision = 0

    init() throws {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        engine = try MusliEngine(dataDir: support.appendingPathComponent("Musli").path)
        downloads = ModelDownloads(engine: engine)
        settings = engine.settings()
        downloads.onModelsChanged = { [weak self] in
            guard let self else { return }
            self.settings = self.engine.settings()
        }
    }

    func save() {
        do {
            try engine.updateSettings(settings: settings)
            settings = engine.settings()
            lastError = nil
        } catch {
            lastError = "Instellingen bewaren mislukt: \(error)"
        }
    }

    func keys() -> ApiKeys {
        var byProvider: [String: String] = [:]
        for account in settings.providers {
            if let key = Keychain.get(account.id) { byProvider[account.id] = key }
        }
        return ApiKeys(byProvider: byProvider)
    }

    func account(_ id: String) -> ProviderAccount? {
        settings.providers.first { $0.id == id }
    }

    func hasKey(_ account: ProviderAccount) -> Bool {
        _ = keyRevision
        return Keychain.get(account.id) != nil
    }

    func setKey(_ key: String, for account: ProviderAccount) {
        guard key != (Keychain.get(account.id) ?? "") else { return }
        Keychain.set(key, for: account.id)
        keyRevision += 1
    }

    func isReady(_ choice: ModelChoice) -> Bool {
        guard let account = account(choice.providerId) else { return false }
        if account.kind == .local { return downloads.states[choice.model] == .downloaded }
        return !requiresKey(kind: account.kind) || hasKey(account)
    }

    var needsOnboarding: Bool {
        Permissions.microphone != .authorized || !Permissions.accessibilityTrusted || !isReady(settings.dictationStt)
    }
}
