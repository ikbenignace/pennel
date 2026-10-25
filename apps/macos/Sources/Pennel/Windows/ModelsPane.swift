@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct ModelsPane: View {
    @Bindable var state: AppState

    var body: some View {
        VStack(spacing: 16) {
            TaskModelEditor(
                state: state,
                title: "Dicteren",
                subtitle: "Spraak naar tekst wanneer je de sneltoets gebruikt.",
                task: .dictationStt,
                choice: $state.settings.dictationStt
            )
            TaskModelEditor(
                state: state,
                title: "Opkuisen en samenvatten",
                subtitle: "Taalmodel dat je dictaten opkuist en meetings samenvat.",
                task: .llm,
                choice: $state.settings.llm
            )
            TaskModelEditor(
                state: state,
                title: "Meetings",
                subtitle: "Spraak naar tekst voor opgenomen meetings.",
                task: .meetingStt,
                choice: $state.settings.meetingStt
            )
        }
    }
}

private struct TaskModelEditor: View {
    let state: AppState
    let title: String
    let subtitle: String
    let task: ProviderTask
    @Binding var choice: ModelChoice
    @State private var available: [RemoteModel] = []

    var body: some View {
        PaneSection(title: title, subtitle: subtitle) {
            FieldRow(label: "Provider") {
                PennelPopup(label: "Provider voor \(title)", options: candidates.map { ($0.id, $0.name) }, selection: providerBinding, disabledValues: Set(candidates.filter { !$0.kind.isPlatformSupported }.map(\.id)))
                .frame(maxWidth: 240)
                StatusDot(ok: state.isReady(choice))
            }
            FieldRow(label: "Model") {
                if account?.kind == .local {
                    localPicker
                } else if account?.kind == .appleSpeech {
                    Text("macOS beheert het dicteermodel en de taalbestanden.")
                        .font(.figtree(12)).foregroundStyle(Theme.muted)
                } else if account?.kind == .appleIntelligence {
                    Text("macOS beheert het ingebouwde taalmodel van Apple Intelligence.")
                        .font(.figtree(12)).foregroundStyle(Theme.muted)
                } else {
                    TextField(account.map { defaultModel(kind: $0.kind, task: task) } ?? "model", text: $choice.model)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 240)
                        .help(account?.kind.modelHint ?? "")
                    if !available.isEmpty {
                        RemoteModelPicker(models: available, task: task, selection: $choice.model)
                    }
                }
            }
            if account?.kind == .appleIntelligence, let reason = FoundationLanguageModel.availability().reason {
                Text(reason).font(.figtree(12)).foregroundStyle(Theme.muted)
            }
            if account?.kind != .local, isRouterModel(id: choice.model) {
                Text("Kiest per aanvraag een willekeurig model — opkuisen wordt onbetrouwbaar.")
                    .font(.figtree(11))
                    .foregroundStyle(Theme.accent)
            }
        }
        .task(id: choice.providerId) { await loadModels() }
    }

    private var account: ProviderAccount? { state.account(choice.providerId) }

    private var candidates: [ProviderAccount] {
        state.settings.providers.filter { supports(kind: $0.kind, task: task) }
    }

    @ViewBuilder
    private var localPicker: some View {
        let downloaded = state.downloads.downloaded
        if downloaded.isEmpty {
            Text("Nog geen model gedownload — zie Providers → \(account?.name ?? "Lokaal").")
                .font(.figtree(12))
                .foregroundStyle(Theme.accent)
        } else {
            PennelPopup(label: "Model voor \(title)", options: downloaded.map { ($0.id, $0.name) }, selection: $choice.model)
            .frame(maxWidth: 240)
        }
    }

    private var providerBinding: Binding<String> {
        Binding(
            get: { choice.providerId },
            set: { id in
                guard let account = state.account(id) else { return }
                let model = account.kind == .local
                    ? (state.downloads.downloaded.first?.id ?? defaultModel(kind: .local, task: task))
                    : defaultModel(kind: account.kind, task: task)
                choice = ModelChoice(providerId: id, model: model)
            }
        )
    }

    private func loadModels() async {
        available = []
        guard let account, account.kind != .local, !isCliAgent(kind: account.kind), !account.baseUrl.isEmpty else { return }
        guard !requiresKey(kind: account.kind) || state.hasKey(account) else { return }
        let (baseUrl, key) = (account.baseUrl, Keychain.get(account.id))
        let models = await Task.detached { (try? providerModels(baseUrl: baseUrl, apiKey: key)) ?? [] }.value
        guard !Task.isCancelled else { return }
        available = models
    }
}
