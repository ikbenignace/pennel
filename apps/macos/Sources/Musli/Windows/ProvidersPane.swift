import Foundation
@preconcurrency import MusliCore
import MusliKit
import SwiftUI

extension ProviderKind {
    static let all: [ProviderKind] = [.groq, .openAi, .openRouter, .mistral, .ollama, .lmStudio, .custom]

    var label: String {
        switch self {
        case .openAi: "OpenAI"
        case .openRouter: "OpenRouter"
        case .groq: "Groq"
        case .mistral: "Mistral"
        case .ollama: "Ollama (lokaal)"
        case .lmStudio: "LM Studio (lokaal)"
        case .custom: "Eigen server"
        }
    }
}

struct ProvidersPane: View {
    @Bindable var state: AppState

    var body: some View {
        VStack(spacing: 14) {
            ProviderEditor(
                title: "Dicteren",
                subtitle: "Spraak naar tekst wanneer je de sneltoets gebruikt.",
                task: .dictationStt,
                config: $state.settings.dictationStt
            )
            ProviderEditor(
                title: "Opkuisen en samenvatten",
                subtitle: "Taalmodel dat je dictaten opkuist en later meetings samenvat.",
                task: .llm,
                config: $state.settings.llm
            )
            ProviderEditor(
                title: "Meetings",
                subtitle: "Spraak naar tekst voor opgenomen meetings (komt in een volgende versie).",
                task: .meetingStt,
                config: $state.settings.meetingStt
            )
        }
    }
}

private struct ProviderEditor: View {
    let title: String
    let subtitle: String
    let task: ProviderTask
    @Binding var config: ProviderConfig

    @State private var apiKey = ""
    @State private var testResult: String?
    @State private var testing = false

    var body: some View {
        PaneSection(title: title, subtitle: subtitle) {
            FieldRow(label: "Provider") {
                Picker("", selection: kindBinding) {
                    ForEach(ProviderKind.all, id: \.self) { Text($0.label).tag($0) }
                }
                .labelsHidden()
                .frame(maxWidth: 220)
            }
            FieldRow(label: "Base URL") {
                TextField("https://…", text: $config.baseUrl).textFieldStyle(.roundedBorder)
            }
            FieldRow(label: "Model") {
                TextField("model", text: $config.model).textFieldStyle(.roundedBorder)
            }
            if !config.keychainAccount.isEmpty {
                FieldRow(label: "API-sleutel") {
                    SecureField("Plak je sleutel", text: $apiKey)
                        .textFieldStyle(.roundedBorder)
                        .onChange(of: apiKey) { Keychain.set(apiKey, for: config.keychainAccount) }
                }
            }
            HStack {
                Spacer()
                if let testResult {
                    Text(testResult).font(.figtree(12)).foregroundStyle(Theme.muted).lineLimit(2)
                }
                Button(testing ? "Testen…" : "Test verbinding") { test() }
                    .disabled(testing || config.baseUrl.isEmpty)
            }
        }
        .onAppear { apiKey = Keychain.get(config.keychainAccount) ?? "" }
        .onChange(of: config.keychainAccount) { apiKey = Keychain.get(config.keychainAccount) ?? "" }
    }

    private var kindBinding: Binding<ProviderKind> {
        Binding(
            get: { config.kind },
            set: { config = preset(kind: $0, task: task) }
        )
    }

    private func test() {
        testing = true
        testResult = nil
        let url = URL(string: config.baseUrl.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + "/models")
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
        var request = URLRequest(url: url, timeoutInterval: 10)
        if !apiKey.isEmpty {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
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
}
