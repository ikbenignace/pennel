@preconcurrency import MusliCore
import MusliKit
import SwiftUI

struct LocalModelsView: View {
    @Bindable var state: AppState

    var body: some View {
        PaneSection(title: "Modellen", subtitle: "Draaien volledig op je Mac. Je audio verlaat je computer niet.") {
            ForEach(state.downloads.models, id: \.id) { model in
                row(model)
                if model.id != state.downloads.models.last?.id { Divider().overlay(Theme.line) }
            }
            FieldRow(label: "Vrijgeven na") {
                Picker("", selection: $state.settings.localUnloadMinutes) {
                    Text("1 min").tag(UInt32(1))
                    Text("5 min").tag(UInt32(5))
                    Text("15 min").tag(UInt32(15))
                    Text("1 uur").tag(UInt32(60))
                }
                .labelsHidden()
                .frame(maxWidth: 140)
            }
            Text("Het model wordt geladen bij je eerste dictaat en daarna uit het geheugen gehaald als je het niet gebruikt.")
                .font(.figtree(11))
                .foregroundStyle(Theme.muted)
        }
    }

    private func row(_ model: LocalModel) -> some View {
        let downloads = state.downloads
        let inUse = isInUse(model)
        return VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(model.name).font(.figtree(13, weight: .medium))
                        if inUse {
                            Text("In gebruik")
                                .font(.figtree(10, weight: .medium))
                                .foregroundStyle(Theme.accent)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 1)
                                .background(Capsule().stroke(Theme.accent.opacity(0.5)))
                        }
                    }
                    Text("\(model.description) · \(megabytes(model.sizeBytes))")
                        .font(.figtree(11))
                        .foregroundStyle(Theme.muted)
                }
                Spacer()
                actions(model, inUse: inUse)
            }
            if let progress = downloads.progress[model.id] {
                ProgressView(value: progress).tint(Theme.accent)
            }
            if let error = downloads.errors[model.id] {
                Text(error).font(.figtree(11)).foregroundStyle(Theme.accent)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private func actions(_ model: LocalModel, inUse: Bool) -> some View {
        let downloads = state.downloads
        if downloads.isDownloading(model.id) {
            Button("Annuleren") { downloads.cancel(model.id) }
        } else {
            switch downloads.states[model.id] ?? .notDownloaded {
            case .downloaded:
                HStack(spacing: 8) {
                    if !inUse { Button("Gebruik") { use(model) } }
                    Button("Verwijder") { downloads.delete(model.id) }
                }
            case .partial:
                Button("Verder downloaden") { downloads.start(model.id) }
            case .notDownloaded:
                Button("Downloaden") { downloads.start(model.id) }
            }
        }
    }

    private func isInUse(_ model: LocalModel) -> Bool {
        state.account(state.settings.dictationStt.providerId)?.kind == .local && state.settings.dictationStt.model == model.id
    }

    private func use(_ model: LocalModel) {
        guard let local = state.settings.providers.first(where: { $0.kind == .local }) else { return }
        state.settings.dictationStt = ModelChoice(providerId: local.id, model: model.id)
    }

    private func megabytes(_ bytes: UInt64) -> String {
        "\(bytes / 1_000_000) MB"
    }
}
