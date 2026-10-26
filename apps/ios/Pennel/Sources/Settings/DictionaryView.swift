import PennelMobileKit
import SwiftUI

struct DictionaryView: View {
    @Environment(AppModel.self) private var model
    @State private var term = ""

    var body: some View {
        Form {
            Section {
                HStack {
                    TextField("Bv. Pennel, Kubernetes, Sarah", text: $term)
                        .autocorrectionDisabled()
                        .onSubmit(add)
                    Button("Toevoegen", action: add)
                        .buttonStyle(.borderless)
                        .disabled(term.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                ForEach(model.settings.dictionary, id: \.self) { word in
                    Text(word)
                }
                .onDelete { offsets in
                    let removed = offsets.map { model.settings.dictionary[$0] }
                    model.update { $0.dictionary.removeAll { removed.contains($0) } }
                }
            } footer: {
                Text("Namen en vaktermen die Pennel exact zo moet schrijven.")
            }
            .pennelRow()
        }
        .pennelList()
    }

    private func add() {
        let word = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !word.isEmpty, !model.settings.dictionary.contains(word) else { return }
        model.update { $0.dictionary.append(word) }
        term = ""
    }
}
