import Foundation
@preconcurrency import PennelCore

/// Adds coding-agent CLIs found on this Mac (Claude Code, Codex, Copilot, …) to the providers,
/// so the user's own subscription can clean up and summarize. A provider the user deleted stays deleted.
enum CliAgents {
    private static let dismissedKey = "dismissedCliAgents"

    static func dismiss(_ kind: ProviderKind) {
        var dismissed = UserDefaults.standard.stringArray(forKey: dismissedKey) ?? []
        dismissed.append(String(describing: kind))
        UserDefaults.standard.set(dismissed, forKey: dismissedKey)
    }

    @MainActor
    static func sync(into state: AppState) {
        Task {
            let found = await Task.detached { detectCliAgents() }.value
            let dismissed = Set(UserDefaults.standard.stringArray(forKey: dismissedKey) ?? [])
            var changed = false
            for agent in found where !dismissed.contains(String(describing: agent.kind)) {
                if let index = state.settings.providers.firstIndex(where: { $0.kind == agent.kind }) {
                    if state.settings.providers[index].baseUrl != agent.path {
                        state.settings.providers[index].baseUrl = agent.path
                        changed = true
                    }
                } else {
                    var account = newAccount(kind: agent.kind, existing: state.settings.providers)
                    account.baseUrl = agent.path
                    state.settings.providers.append(account)
                    changed = true
                }
            }
            if changed {
                state.save()
                Diagnostics("providers").notice("CLI's gevonden: \(found.map { $0.kind.label }.joined(separator: ", "))")
            }
        }
    }
}
