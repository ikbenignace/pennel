import PennelMobileKit

/// Labels and order copied from `KINDS` in `apps/desktop/src-tauri/src/commands.rs`.
enum ProviderKinds {
    static let all: [(kind: ProviderKind, label: String)] = [
        (.openAi, "OpenAI"),
        (.openRouter, "OpenRouter"),
        (.groq, "Groq"),
        (.mistral, "Mistral"),
        (.ollama, "Ollama (lokaal)"),
        (.lmStudio, "LM Studio (lokaal)"),
        (.custom, "Eigen server"),
        (.local, "Lokaal op deze computer"),
        (.appleSpeech, "Apple-dicteren"),
        (.appleIntelligence, "Apple Intelligence"),
        (.claudeCode, "Claude Code"),
        (.codex, "Codex"),
        (.openCode, "OpenCode"),
        (.copilot, "GitHub Copilot"),
        (.cursorAgent, "Cursor"),
        (.geminiCli, "Gemini CLI"),
        (.pennelCloud, "Pennel Cloud"),
    ]

    static func label(_ kind: ProviderKind) -> String {
        all.first { $0.kind == kind }?.label ?? ""
    }

    /// Why a kind cannot run on iPhone; nil when it can.
    static func unavailableReason(_ kind: ProviderKind) -> String? {
        if isCliAgent(kind: kind) { return Copy.Settings.cliAgentsDisabled }
        if kind == .local { return Copy.Settings.localModelsDisabled }
        return nil
    }

    static func usesBaseURL(_ kind: ProviderKind) -> Bool {
        !isCliAgent(kind: kind) && ![.local, .appleSpeech, .appleIntelligence, .pennelCloud].contains(kind)
    }

    /// Kinds that can only be added once, as on the desktop.
    static func isSingleton(_ kind: ProviderKind) -> Bool {
        [.local, .appleSpeech, .appleIntelligence].contains(kind)
    }
}
