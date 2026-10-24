import Foundation
@preconcurrency import PennelCore

/// One-time move of everything the app stored as "Musli" before it was renamed to Pennel.
enum LegacyMigration {
    private static let marker = "migrated-from-musli"

    /// Runs before the engine opens its data folder, so settings, history and models come along.
    static func moveDataFolder(in support: URL) {
        let fm = FileManager.default
        let old = support.appendingPathComponent("Musli")
        let new = support.appendingPathComponent("Pennel")
        guard fm.fileExists(atPath: old.path), !fm.fileExists(atPath: new.path) else { return }
        do {
            try fm.moveItem(at: old, to: new)
            try Data().write(to: new.appendingPathComponent(marker))
        } catch {
            Diagnostics("app").error("gegevens van Musli verhuizen mislukt: \(error)")
        }
    }

    /// Keeps meetings where they are (`~/Musli`) instead of silently starting an empty `~/Pennel`,
    /// and copies API keys to the new Keychain service.
    @MainActor
    static func finish(state: AppState, dataDir: URL) {
        let markerFile = dataDir.appendingPathComponent(marker)
        guard FileManager.default.fileExists(atPath: markerFile.path) else { return }
        defer { try? FileManager.default.removeItem(at: markerFile) }

        let home = URL(fileURLWithPath: NSHomeDirectory())
        let oldMeetings = home.appendingPathComponent("Musli")
        let newMeetings = home.appendingPathComponent("Pennel")
        if state.settings.meetingsDir.isEmpty,
           FileManager.default.fileExists(atPath: oldMeetings.path),
           !FileManager.default.fileExists(atPath: newMeetings.path) {
            state.settings.meetingsDir = oldMeetings.path
            state.save()
        }
        for account in state.settings.providers where Keychain.get(account.id) == nil {
            if let key = Keychain.legacyValue(account.id) { Keychain.set(key, for: account.id) }
        }
        Diagnostics("app").notice("gegevens van Musli overgenomen")
    }
}
