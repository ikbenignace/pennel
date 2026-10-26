import PennelMobileKit
import SwiftUI

struct ModelsView: View {
    var body: some View {
        Form {
            ModelSlot(task: .dictationStt, slot: \.dictationStt, title: "Dicteren", subtitle: nil)
            ModelSlot(task: .llm, slot: \.llm, title: "Opkuisen en samenvatten",
                      subtitle: "Taalmodel dat je dictaten opkuist en meetings samenvat.")
            ModelSlot(task: .meetingStt, slot: \.meetingStt, title: "Meetings",
                      subtitle: "Spraak naar tekst voor opgenomen meetings.")
            Section {
                Text("Lokale modellen")
                    .foregroundStyle(Brand.disabledText.color)
            } footer: {
                Text(Copy.Settings.localModelsDisabled)
            }
            .pennelRow()
        }
        .pennelList()
    }
}

private struct ModelSlot: View {
    @Environment(AppModel.self) private var model
    let task: ProviderTask
    let slot: WritableKeyPath<Settings, ModelChoice>
    let title: String
    let subtitle: String?

    @State private var modelName = ""
    @State private var testing = false
    @State private var outcome: ProviderTaskTestResult?

    private var choice: ModelChoice { model.settings[keyPath: slot] }
    private var account: ProviderAccount? { model.settings.providers.first { $0.id == choice.providerId } }
    private var eligible: [ProviderAccount] { model.settings.providers.filter { supports(kind: $0.kind, task: task) } }

    private var configured: Bool {
        guard let account, ProviderKinds.unavailableReason(account.kind) == nil else { return false }
        if requiresKey(kind: account.kind) && !model.hasKey(for: account.id) { return false }
        return !choice.model.trimmingCharacters(in: .whitespaces).isEmpty
    }

    var body: some View {
        Section {
            LabeledContent("Provider") {
                Menu {
                    ForEach(eligible, id: \.id) { provider in
                        let reason = ProviderKinds.unavailableReason(provider.kind)
                        Button {
                            choose(provider)
                        } label: {
                            if provider.id == choice.providerId {
                                Label(provider.name, systemImage: "checkmark")
                            } else {
                                Text(reason.map { "\(provider.name) — \($0)" } ?? provider.name)
                            }
                        }
                        .disabled(reason != nil)
                    }
                } label: {
                    Text(account?.name ?? "Kies een provider")
                }
            }
            if let account, ProviderKinds.usesBaseURL(account.kind) || account.kind == .pennelCloud {
                TextField("Model", text: $modelName, prompt: Text("modelnaam"))
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onSubmit(saveModel)
            }
            HStack {
                Text(readiness)
                    .foregroundStyle(Brand.muted.color)
                Spacer()
                Button(testing ? "Testen…" : task == .llm ? "Test tekstverwerking" : "Test spraakherkenning", action: test)
                    .buttonStyle(.borderless)
                    .disabled(testing || !configured)
            }
        } header: {
            Text(title)
        } footer: {
            VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                if let subtitle { Text(subtitle) }
                if let reason = account.flatMap({ ProviderKinds.unavailableReason($0.kind) }) { Text(reason) }
                if let outcome { Text(outcome.message) }
            }
        }
        .pennelRow()
        .onAppear { modelName = choice.model }
        .onChange(of: choice) { modelName = choice.model }
        .onDisappear(perform: saveModel)
    }

    private var readiness: String {
        !configured ? "Nog instellen" : outcome?.verified == true ? "Test geslaagd" : "Ingesteld"
    }

    private func choose(_ provider: ProviderAccount) {
        let defaultName = defaultModel(kind: provider.kind, task: task)
        outcome = nil
        model.update { $0[keyPath: slot] = ModelChoice(providerId: provider.id, model: defaultName) }
    }

    private func saveModel() {
        let name = modelName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard name != choice.model else { return }
        outcome = nil
        model.update { $0[keyPath: slot].model = name }
    }

    private func test() {
        saveModel()
        testing = true
        let engine = model.engine
        let task = task
        let keys = Keychain.apiKeys()
        Task {
            let result = await Task.detached { () -> ProviderTaskTestResult in
                do {
                    return try testProviderTask(engine: engine, task: task, keys: keys)
                } catch {
                    return ProviderTaskTestResult(verified: false, message: ErrorText.failed(error))
                }
            }.value
            outcome = result
            testing = false
        }
    }
}
