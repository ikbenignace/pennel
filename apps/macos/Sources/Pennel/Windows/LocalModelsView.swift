@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct LocalModelsView: View {
    @Bindable var state: AppState
    @State private var showAll = false
    @State private var pendingDelete: LocalModel?
    @State private var languageInitialized = false

    var body: some View {
        let downloads = state.downloads
        PaneSection(title: "Modellen", subtitle: "Draaien volledig op je computer. Je audio verlaat je computer niet.") {
            HStack(spacing: 12) {
                HStack(spacing: 12) {
                    Text("Nauwkeurigheid voor").foregroundStyle(Theme.muted).frame(width: 90, alignment: .leading)
                    PennelPopup(label: "Nauwkeurigheid voor", options: LanguageNames.common.map { ($0, LanguageNames.name($0)) }, selection: languageBinding)
                }
                .frame(maxWidth: 260)
                Toggle("Alleen deze taal", isOn: supportedOnlyBinding)
                    .toggleStyle(.checkbox)
            }
            .font(.figtree(12))
            Text("Scores uit onafhankelijke benchmarks: snelheid op Apple Silicon, nauwkeurigheid in de gekozen taal (hoger is beter).")
                .font(.figtree(11))
                .foregroundStyle(Theme.muted)

            let recommended = downloads.visible.filter { $0.recommended || isDownloaded($0) }
            let others = downloads.visible.filter { !$0.recommended && !isDownloaded($0) }
            list(recommended)
            if !others.isEmpty {
                DisclosureGroup(isExpanded: $showAll) {
                    list(others)
                } label: {
                    Text("Alle modellen (\(others.count))").font(.figtree(12, weight: .medium))
                }
            }

            FieldRow(label: "Vrijgeven na") {
                PennelPopup(label: "Vrijgeven na", options: [(UInt32(1), "1 min"), (UInt32(5), "5 min"), (UInt32(15), "15 min"), (UInt32(60), "1 uur")], selection: $state.settings.localUnloadMinutes)
                .frame(maxWidth: 140)
            }
            Text("Het model wordt geladen bij je eerste dictaat en daarna uit het geheugen gehaald als je het niet gebruikt.")
                .font(.figtree(11))
                .foregroundStyle(Theme.muted)
        }
        .onAppear {
            guard !languageInitialized else { return }
            languageInitialized = true
            downloads.scoreLanguage = state.settings.language ?? "nl"
        }
        .confirmationDialog(
            "Dit model is in gebruik. Verwijderen?",
            isPresented: Binding(get: { pendingDelete != nil }, set: { if !$0 { pendingDelete = nil } }),
            presenting: pendingDelete
        ) { model in
            Button("Verwijder", role: .destructive) { downloads.delete(model.id) }
        } message: { _ in
            Text("Dicteren schakelt over naar een ander gedownload model, of werkt niet meer tot je er een downloadt.")
        }
    }

    private func list(_ models: [LocalModel]) -> some View {
        ForEach(models, id: \.id) { model in
            ModelCard(
                model: model,
                inUse: isInUse(model),
                progress: state.downloads.progress[model.id],
                error: state.downloads.errors[model.id]
            ) {
                actions(model)
            }
            if model.id != models.last?.id { Divider().overlay(Theme.line) }
        }
    }

    private var languageBinding: Binding<String> {
        Binding(get: { state.downloads.scoreLanguage }, set: { state.downloads.scoreLanguage = $0 })
    }

    private var supportedOnlyBinding: Binding<Bool> {
        Binding(get: { state.downloads.supportedOnly }, set: { state.downloads.supportedOnly = $0 })
    }

    @ViewBuilder
    private func actions(_ model: LocalModel) -> some View {
        let downloads = state.downloads
        if downloads.isDownloading(model.id) {
            Button("Annuleren") { downloads.cancel(model.id) }
        } else {
            switch downloads.states[model.id] ?? .notDownloaded {
            case .downloaded:
                HStack(spacing: 8) {
                    if !isInUse(model) { Button("Gebruik") { use(model) } }
                    Button("Verwijder") { delete(model) }
                }
            case .partial:
                HStack(spacing: 8) {
                    Button("Verder downloaden") { downloads.start(model.id) }
                    Button("Verwijder") { downloads.delete(model.id) }
                }
            case .notDownloaded:
                Button("Downloaden") { downloads.start(model.id) }
            }
        }
    }

    private func isDownloaded(_ model: LocalModel) -> Bool {
        state.downloads.states[model.id] == .downloaded
    }

    private func isInUse(_ model: LocalModel) -> Bool {
        [state.settings.dictationStt, state.settings.meetingStt].contains { choice in
            state.account(choice.providerId)?.kind == .local && choice.model == model.id
        }
    }

    private func delete(_ model: LocalModel) {
        if isInUse(model) { pendingDelete = model } else { state.downloads.delete(model.id) }
    }

    private func use(_ model: LocalModel) {
        guard let local = state.settings.providers.first(where: { $0.kind == .local }) else { return }
        state.settings.dictationStt = ModelChoice(providerId: local.id, model: model.id)
    }
}
