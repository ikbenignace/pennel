import Foundation
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

extension ProviderKind {
    static let all: [ProviderKind] = [.groq, .openAi, .openRouter, .mistral, .ollama, .lmStudio, .custom, .local]

    var label: String {
        switch self {
        case .openAi: "OpenAI"
        case .openRouter: "OpenRouter"
        case .groq: "Groq"
        case .mistral: "Mistral"
        case .ollama: "Ollama (lokaal)"
        case .lmStudio: "LM Studio (lokaal)"
        case .custom: "Eigen server"
        case .local: "Lokaal op deze Mac"
        case .claudeCode: "Claude Code"
        case .codex: "Codex (ChatGPT)"
        case .openCode: "OpenCode"
        case .copilot: "GitHub Copilot"
        case .cursorAgent: "Cursor"
        case .geminiCli: "Gemini CLI"
        case .appleSpeech: "Apple-dicteren (op deze Mac)"
        }
    }

    /// Hint shown under the model field; empty means the CLI's own default.
    var modelHint: String {
        switch self {
        case .claudeCode: "haiku is snel genoeg om op te kuisen; sonnet of opus voor samenvattingen"
        case .codex, .openCode, .copilot, .cursorAgent, .geminiCli: "Leeg = het model dat in de CLI zelf ingesteld is"
        default: ""
        }
    }
}

struct ProvidersPane: View {
    @Bindable var state: AppState
    @State private var selection: String?

    var body: some View {
        HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(state.settings.providers, id: \.id) { account in
                    ProviderRow(account: account, ready: ready(account), selected: selection == account.id)
                        .onTapGesture { selection = account.id }
                }
                Menu("Provider toevoegen") {
                    ForEach(addableKinds, id: \.self) { kind in
                        Button(kind.label) { add(kind) }
                    }
                }
                .padding(.top, 8)
            }
            .frame(width: 190, alignment: .leading)

            Group {
                if let selection, let account = state.account(selection) {
                    ProviderDetail(
                        state: state,
                        account: binding(for: selection, fallback: account),
                        canDelete: state.settings.providers.count > 1,
                        onDelete: { delete(account) }
                    )
                    .id(selection)
                } else {
                    Text("Kies een provider").foregroundStyle(Theme.muted).frame(maxWidth: .infinity, minHeight: 200)
                }
            }
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .onAppear { selection = selection ?? state.settings.providers.first?.id }
    }

    /// Looks the provider up by id on every access, so a deleted provider can't index past the array.
    private func binding(for id: String, fallback: ProviderAccount) -> Binding<ProviderAccount> {
        Binding(
            get: { state.account(id) ?? fallback },
            set: { updated in
                guard let index = state.settings.providers.firstIndex(where: { $0.id == id }) else { return }
                state.settings.providers[index] = updated
            }
        )
    }

    private var addableKinds: [ProviderKind] {
        let present = Set(state.settings.providers.map(\.kind).filter { $0 == .local || $0 == .appleSpeech })
        var kinds = ProviderKind.all
        if #available(macOS 26, *) { kinds.append(.appleSpeech) }
        return kinds.filter { !present.contains($0) }
    }

    private func ready(_ account: ProviderAccount) -> Bool {
        if account.kind == .local { return !state.downloads.downloaded.isEmpty }
        if account.kind == .appleSpeech {
            if #available(macOS 26, *) { return true }
            return false
        }
        if isCliAgent(kind: account.kind) { return FileManager.default.isExecutableFile(atPath: account.baseUrl) }
        return !requiresKey(kind: account.kind) || state.hasKey(account)
    }

    private func add(_ kind: ProviderKind) {
        let account = newAccount(kind: kind, existing: state.settings.providers)
        state.settings.providers.append(account)
        selection = account.id
    }

    private func delete(_ account: ProviderAccount) {
        if isCliAgent(kind: account.kind) { CliAgents.dismiss(account.kind) }
        Keychain.set(nil, for: account.id)
        state.settings.providers.removeAll { $0.id == account.id }
        state.save()
        selection = state.settings.providers.first?.id
    }
}

private struct ProviderRow: View {
    let account: ProviderAccount
    let ready: Bool
    let selected: Bool

    var body: some View {
        HStack(spacing: 8) {
            StatusDot(ok: ready)
            VStack(alignment: .leading, spacing: 1) {
                Text(account.name).font(.figtree(13, weight: .medium))
                Text(account.kind.label).font(.figtree(11)).foregroundStyle(Theme.muted)
            }
            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(RoundedRectangle(cornerRadius: 8).fill(selected ? Theme.line : .clear))
        .contentShape(Rectangle())
    }
}

private struct ProviderDetail: View {
    let state: AppState
    @Binding var account: ProviderAccount
    let canDelete: Bool
    let onDelete: () -> Void

    @State private var apiKey = ""
    @State private var testResult: String?
    @State private var testing = false

    var body: some View {
        VStack(spacing: 14) {
            PaneSection(title: account.kind.label) {
                FieldRow(label: "Naam") {
                    TextField("Naam", text: $account.name).textFieldStyle(.roundedBorder)
                }
                if isCliAgent(kind: account.kind) {
                    CliAgentDetail(account: account)
                } else if account.kind == .appleSpeech {
                    Text("Het dicteermodel van macOS zelf. Gratis, volledig offline en zonder download: macOS beheert de taalbestanden. Werkt voor dicteren en meetings vanaf macOS 26.")
                        .font(.figtree(12))
                        .foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                } else if account.kind != .local {
                    FieldRow(label: "Base URL") {
                        TextField("https://…", text: $account.baseUrl).textFieldStyle(.roundedBorder)
                    }
                    FieldRow(label: requiresKey(kind: account.kind) ? "API-sleutel" : "Sleutel (optioneel)") {
                        SecureField("Plak je sleutel", text: $apiKey)
                            .textFieldStyle(.roundedBorder)
                            .onChange(of: apiKey) { state.setKey(apiKey, for: account) }
                    }
                    HStack {
                        if let testResult {
                            Text(testResult).font(.figtree(12)).foregroundStyle(Theme.muted).lineLimit(2)
                        }
                        Spacer()
                        Button(testing ? "Testen…" : "Test verbinding", action: test)
                            .disabled(testing || account.baseUrl.isEmpty)
                    }
                }
            }
            if account.kind == .local {
                LocalModelsView(state: state)
            }
            HStack {
                Spacer()
                Button("Provider verwijderen", role: .destructive, action: onDelete).disabled(!canDelete)
            }
        }
        .onAppear { apiKey = Keychain.get(account.id) ?? "" }
    }

    private func test() {
        testing = true
        testResult = nil
        let url = URL(string: account.baseUrl.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + "/models")
        let key = apiKey
        Task {
            testResult = await ConnectionTest.run(url: url, apiKey: key)
            testing = false
        }
    }
}

enum ConnectionTest {
    static func run(url: URL?, apiKey: String) async -> String {
        guard let url else { return "Ongeldige URL" }
        do {
            let (_, response) = try await URLSession.shared.data(for: request(url, apiKey))
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            switch status {
            case 200..<300: return "✓ Verbonden"
            case 401, 403: return "API-sleutel geweigerd (\(status))"
            default: return "Server antwoordde \(status)"
            }
        } catch {
            return "Geen verbinding: \(error.localizedDescription)"
        }
    }

    private static func request(_ url: URL, _ apiKey: String) -> URLRequest {
        var request = URLRequest(url: url, timeoutInterval: 10)
        if !apiKey.isEmpty { request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization") }
        return request
    }
}

private struct CliAgentDetail: View {
    let account: ProviderAccount
    @State private var result: String?
    @State private var checking = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Gebruikt je eigen abonnement via de CLI op deze Mac, voor opkuisen, samenvatten en vragen. Pennel draait hem in een lege map, zonder tools. Trager dan een API (enkele seconden per opdracht).")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
            FieldRow(label: "Programma") {
                Text(account.baseUrl.isEmpty ? "Niet gevonden" : account.baseUrl)
                    .font(.figtree(12))
                    .textSelection(.enabled)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            HStack {
                if let result {
                    Text(result).font(.figtree(12)).foregroundStyle(Theme.muted).lineLimit(3)
                }
                Spacer()
                Button(checking ? "Testen…" : "Test", action: check).disabled(checking || account.baseUrl.isEmpty)
            }
        }
    }

    private func check() {
        checking = true
        result = nil
        let (kind, path) = (account.kind, account.baseUrl)
        let model = defaultModel(kind: kind, task: .llm)
        Task {
            let outcome = await Task.detached { Result { try checkCliAgent(kind: kind, path: path, model: model) } }.value
            checking = false
            switch outcome {
            case .success: result = "✓ Werkt"
            case .failure(let error):
                result = "Mislukt: \(error)"
                Diagnostics("providers").error("\(kind.label) test mislukt: \(error)")
            }
        }
    }
}
