@preconcurrency import MusliCore
import MusliKit
import SwiftUI

struct TemplatesSection: View {
    let state: AppState
    @State private var templates: [MeetingTemplate] = []
    @State private var editing: EditorTarget?
    @State private var deleting: MeetingTemplate?

    struct EditorTarget {
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
            Button("Nieuw sjabloon") {
                editing = EditorTarget(id: nil, name: "", body: "Beschrijf waar de samenvatting op moet letten.\n\n## Samenvatting\n## Actiepunten\n")
            }
        }
        .onAppear(perform: reload)
        .sheet(item: Binding(get: { editing.map(Wrapped.init) }, set: { editing = $0?.target })) { wrapped in
            TemplateEditor(target: wrapped.target) { name, body in
                _ = try? state.engine.saveTemplate(id: wrapped.target.id, name: name, body: body)
                editing = nil
                reload()
            } onCancel: {
                editing = nil
            }
        }
        .confirmationDialog(
            deleting?.builtin == true ? "Origineel sjabloon herstellen?" : "Sjabloon \(deleting?.name ?? "") verwijderen?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            presenting: deleting
        ) { template in
            Button(template.builtin ? "Herstel" : "Verwijder", role: .destructive) {
                try? state.engine.deleteTemplate(id: template.id)
                if state.settings.defaultTemplate == template.id && !template.builtin {
                    state.settings.defaultTemplate = "algemeen"
                }
                reload()
            }
        }
    }

    private struct Wrapped: Identifiable {
        let target: EditorTarget
        var id: String { target.identity }
    }

    private func tag(_ text: String) -> some View {
        Text(text).font(.figtree(10)).foregroundStyle(Theme.muted)
    }

    private func edit(_ template: MeetingTemplate) {
        let body = state.engine.templateSource(id: template.id) ?? ""
        editing = EditorTarget(id: template.id, name: template.name, body: body)
    }

    private func reload() {
        templates = state.engine.meetingTemplates()
    }
}

struct TemplateEditor: View {
    let target: TemplatesSection.EditorTarget
    let onSave: (String, String) -> Void
    let onCancel: () -> Void
    @State private var name = ""
    @State private var text = ""

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
            HStack {
                Spacer()
                Button("Annuleren", action: onCancel).keyboardShortcut(.cancelAction)
                Button("Bewaar") { onSave(name.trimmingCharacters(in: .whitespaces), text) }
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
