import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct MeetingsPane: View {
    @Bindable var state: AppState

    var body: some View {
        VStack(spacing: 16) {
            PaneSection(title: "Map", subtitle: "Elke meeting wordt een map met Markdown en audio. Leesbaar in Obsidian.") {
                HStack {
                    Text(displayPath).lineLimit(1).truncationMode(.middle)
                    Spacer()
                    Button("Kies…", action: choose).disabled(state.preview)
                    if !state.settings.meetingsDir.isEmpty {
                        Button("Standaard") { state.settings.meetingsDir = "" }.disabled(state.preview)
                    }
                }
                if state.preview { Text("De preview gebruikt uitsluitend zijn eigen meetingmap.").font(.figtree(11)).foregroundStyle(Theme.muted) }
                if syncsToCloud {
                    Text("Deze map lijkt naar iCloud te synchroniseren. Je meetings en audio gaan dan ook naar Apple.")
                        .font(.figtree(12))
                        .foregroundStyle(Theme.accent)
                }
            }
            PaneSection(title: "Standaardsjabloon") {
                TemplatePicker(state: state, selection: $state.settings.defaultTemplate)
            }
            TemplatesSection(state: state)
            SpeakersSection(state: state)
            PaneSection(title: "Audio bewaren", subtitle: "Notities en transcript blijven altijd staan.") {
                PennelPopup(label: "Audio bewaren", options: [(UInt32(30), "30 dagen"), (UInt32(90), "90 dagen"), (UInt32(365), "1 jaar"), (UInt32(0), "Altijd")], selection: $state.settings.meetingAudioRetentionDays)
                .frame(maxWidth: 220)
            }
        }
    }

    private var path: String {
        state.settings.meetingsDir.isEmpty ? NSHomeDirectory() + "/Pennel" : state.settings.meetingsDir
    }

    private var displayPath: String {
        state.preview ? "Preview · aparte meetingmap" : path.replacingOccurrences(of: NSHomeDirectory(), with: "~")
    }

    /// iCloud Drive lives under Mobile Documents; Desktop & Documents sync when that iCloud option is on.
    private var syncsToCloud: Bool {
        let home = NSHomeDirectory()
        return path.contains("/Library/Mobile Documents/")
            || path.hasPrefix(home + "/Documents")
            || path.hasPrefix(home + "/Desktop")
    }

    private func choose() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Kies"
        if panel.runModal() == .OK, let url = panel.url {
            state.settings.meetingsDir = url.path
        }
    }
}

struct TemplatePicker: View {
    let state: AppState
    @Binding var selection: String

    var body: some View {
        PennelPopup(label: "Sjabloon", options: state.templates.map { ($0.id, $0.name) }, selection: $selection)
        .frame(maxWidth: 220)
        .onAppear {
            if state.templates.isEmpty { state.reloadTemplates() }
            selection = state.templateId(selection)
        }
        .onChange(of: state.templates.map(\.id)) { selection = state.templateId(selection) }
    }
}

private struct SpeakersSection: View {
    @Bindable var state: AppState
    @State private var modelState: ModelState = .notDownloaded

    private var modelId: String { state.engine.diarizerModelId() }

    var body: some View {
        PaneSection(
            title: "Sprekers herkennen",
            subtitle: "Splitst \"Anderen\" na de meeting op in afzonderlijke stemmen die je een naam kunt geven. Draait lokaal."
        ) {
            Toggle("Sprekers herkennen na elke meeting", isOn: $state.settings.diarization)
                .font(.figtree(12))
            HStack {
                Text(status).font(.figtree(11)).foregroundStyle(Theme.muted)
                Spacer()
                if state.downloads.isDownloading(modelId) {
                    Button("Annuleren") { state.downloads.cancel(modelId) }
                } else if modelState != .downloaded {
                    Button("Download model (139 MB)") { state.downloads.start(modelId, sizeBytes: 139_310_336) }
                }
            }
            if let progress = state.downloads.progress[modelId] {
                ProgressView(value: progress).tint(Theme.accent)
            }
            if let error = state.downloads.errors[modelId] {
                Text(error).font(.figtree(11)).foregroundStyle(Theme.accent)
            }
        }
        .onAppear(perform: refresh)
        .onChange(of: state.downloads.progress[modelId] == nil) { refresh() }
    }

    private var status: String {
        switch modelState {
        case .downloaded: "Model klaar · Sortformer, tot 4 sprekers"
        case .partial: "Download onderbroken"
        case .notDownloaded: "Model nog niet gedownload"
        }
    }

    private func refresh() {
        modelState = state.engine.modelState(id: modelId)
    }
}
