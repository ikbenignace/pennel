@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct TemplatesSection: View {
    let state: AppState
    private var editing: EditorTarget? {
        get { state.workspace.templateEditing }
        nonmutating set { state.workspace.templateEditing = newValue }
    }
    @State private var deleting: MeetingTemplate?
    @State private var error: String?

    private var templates: [MeetingTemplate] { state.templates }

    struct EditorTarget: Identifiable {
        let id: String?
        let name: String
        let body: String
        var identity: String { id ?? "new" }
    }

    var body: some View {
        PaneSection(title: "Sjablonen", subtitle: "Bepalen welke secties een samenvatting krijgt. Bewaard als Markdown in je meetingmap.") {
            ForEach(templates, id: \.id) { template in
                HStack {
                    Text(template.name).font(.figtree(13))
                    if !template.builtin { tag("eigen") } else if template.customized { tag("aangepast") }
                    Spacer()
                    Button("Bewerk") { edit(template) }
                    if !template.builtin || template.customized {
                        Button(template.builtin ? "Herstel" : "Verwijder") { deleting = template }
                    }
                }
                if template.id != templates.last?.id { Divider().overlay(Theme.line) }
            }
            if let error {
                Text(error).font(.figtree(11)).foregroundStyle(Theme.accent)
            }
            Button("Nieuw sjabloon") {
                editing = EditorTarget(id: nil, name: "", body: "Beschrijf waar de samenvatting op moet letten.\n\n## Samenvatting\n## Actiepunten\n")
            }
        }
        .onAppear(perform: reload)
        .confirmationDialog(
            deleting?.builtin == true ? "Origineel sjabloon herstellen?" : "Sjabloon \(deleting?.name ?? "") verwijderen?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { template in
            Button(template.builtin ? "Herstel" : "Verwijder", role: .destructive) {
                do {
                    try state.engine.deleteTemplate(id: template.id)
                    error = nil
                    reload()
                    state.settings.defaultTemplate = state.templateId(state.settings.defaultTemplate)
                } catch {
                    self.error = "Verwijderen mislukt: \(error)"
                }
            }
        }
    }


    private func tag(_ text: String) -> some View {
        Text(text).font(.figtree(11)).foregroundStyle(Theme.muted)
    }

    private func edit(_ template: MeetingTemplate) {
        let body = state.engine.templateSource(id: template.id) ?? ""
        editing = EditorTarget(id: template.id, name: template.name, body: body)
    }

    private func reload() {
        state.reloadTemplates()
    }
}

struct TemplateEditor: View {
    let target: TemplatesSection.EditorTarget
    /// Returns an error message to show, or nil when saved.
    let onSave: (String, String) -> String?
    let onCancel: () -> Void
    @State private var name = ""
    @State private var text = ""
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(target.id == nil ? "Nieuw sjabloon" : "Sjabloon bewerken").font(.figtree(15, weight: .semibold))
            TextField("Naam", text: $name).textFieldStyle(.roundedBorder)
            TextEditor(text: $text)
                .font(.system(size: 12, design: .monospaced))
                .frame(minHeight: 220)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
            Text("Schrijf wat de samenvatting moet doen. Elke regel die met ## begint wordt een vaste sectie.")
                .font(.figtree(11))
                .foregroundStyle(Theme.muted)
            Text("Secties: " + sections.joined(separator: " · "))
                .font(.figtree(11))
            if let error {
                Text(error).font(.figtree(11)).foregroundStyle(Theme.accent)
            }
            HStack {
                Spacer()
                Button("Annuleren", action: onCancel).keyboardShortcut(.cancelAction)
                Button("Bewaar") { error = onSave(name.trimmingCharacters(in: .whitespaces), text) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 520)
        .onAppear {
            name = target.name
            text = target.body
        }
    }

    private var sections: [String] {
        let found = text.split(separator: "\n").compactMap { line -> String? in
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix("## ") else { return nil }
            let title = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
            return title.isEmpty ? nil : title
        }
        return found.isEmpty ? ["Samenvatting", "Beslissingen", "Actiepunten"] : found
    }
}
