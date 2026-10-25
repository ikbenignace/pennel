@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct RemoteModelPicker: View {
    let models: [RemoteModel]
    let task: ProviderTask
    @Binding var selection: String
    @State private var isOpen = false
    @State private var query = ""

    var body: some View {
        Button("Kies…") { isOpen = true }
            .popover(isPresented: $isOpen, arrowEdge: .bottom) {
                VStack(alignment: .leading, spacing: 8) {
                    TextField("Zoek model", text: $query)
                        .textFieldStyle(.roundedBorder)
                    if matches.isEmpty {
                        Text("Geen modellen gevonden").font(.figtree(12)).foregroundStyle(Theme.muted)
                    }
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(matches, id: \.id) { model in
                                row(model)
                            }
                        }
                    }
                }
                .padding(12)
                .frame(width: 420, height: 360)
            }
    }

    private var matches: [RemoteModel] {
        let fitting = models.filter { task == .llm ? !$0.audioInput || $0.contextLength != nil : $0.audioInput }
        let words = query.lowercased().split(separator: " ")
        return fitting.filter { model in
            let haystack = (model.id + " " + (model.name ?? "")).lowercased()
            return words.allSatisfy { haystack.contains($0) }
        }
    }

    private func row(_ model: RemoteModel) -> some View {
        Button {
            selection = model.id
            isOpen = false
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(model.id).font(.figtree(12, weight: model.id == selection ? .semibold : .regular))
                    if model.isRouter {
                        Text("willekeurig").font(.figtree(11)).foregroundStyle(Theme.accent)
                    }
                }
                if let facts = facts(model) {
                    Text(facts).font(.figtree(11)).foregroundStyle(Theme.muted)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 5)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func facts(_ model: RemoteModel) -> String? {
        var parts: [String] = []
        if let context = model.contextLength { parts.append(Self.context(context)) }
        if let input = model.promptPricePerMillion, let output = model.completionPricePerMillion {
            parts.append(input == 0 && output == 0 ? "gratis" : "$\(Self.price(input)) / $\(Self.price(output)) per 1M tokens")
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    private static func context(_ tokens: UInt32) -> String {
        tokens >= 1_000_000 ? "\(tokens / 1_000_000)M context" : "\(tokens / 1000)k context"
    }

    private static func price(_ value: Double) -> String {
        value.formatted(.number.precision(.fractionLength(0...3)).locale(Locale(identifier: "nl_BE")))
    }
}
