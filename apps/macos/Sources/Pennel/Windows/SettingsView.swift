@preconcurrency import PennelCore
import PennelKit
import SwiftUI

enum SettingsTab: String, Hashable, CaseIterable, Identifiable {
    case providers, models, dictation, meetings, recording, dictionary, styles, appearance
    var id: Self { self }
    var title: String {
        switch self {
        case .providers: "Providers"
        case .models: "Modellen"
        case .dictation: "Dicteren"
        case .meetings: "Meetings"
        case .recording: "Opnemen"
        case .dictionary: "Woordenlijst"
        case .styles: "Stijlen"
        case .appearance: "Weergave"
        }
    }
}

struct SettingsView: View {
    @Bindable var state: AppState
    let calendar: CalendarProvider
    private var tab: SettingsTab { state.workspace.settingsTab }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Instellingen").font(.figtree(21, weight: .semibold)).padding(.bottom, 26)
            HStack(alignment: .top, spacing: 28) {
                VStack(spacing: 3) {
                    ForEach(SettingsTab.allCases) { item in
                        Button { state.workspace.settingsTab = item } label: {
                            Text(item.title).font(.figtree(12, weight: tab == item ? .medium : .regular))
                                .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 9).padding(.vertical, 7)
                                .background(RoundedRectangle(cornerRadius: 6).fill(tab == item ? Theme.selection : .clear))
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(tab == item ? .isSelected : [])
                    }
                }
                .frame(width: 124)
                ScrollView {
                Group {
                    switch tab {
                    case .providers: ProvidersPane(state: state)
                    case .models: ModelsPane(state: state)
                    case .dictation: DictationPane(state: state)
                    case .meetings: MeetingsPane(state: state)
                    case .recording: RecordingPane(state: state, calendar: calendar)
                    case .dictionary: DictionaryPane(state: state)
                    case .styles: StylesPane(state: state)
                    case .appearance: AppearancePane(state: state)
                    }
                }
                .padding(.bottom, 24)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            if let error = state.lastError {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent).padding(8)
            }
            HStack(spacing: 10) {
                Text("Pennel \(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.1.0")\(state.preview ? " · Preview" : "")")
                    .font(.figtree(11)).foregroundStyle(Theme.muted)
                Spacer()
                if state.preview { Text("Updates uitgeschakeld in de preview.").font(.figtree(11)).foregroundStyle(Theme.muted) }
                Button("Zoek naar updates…") { state.onCheckUpdates?() }.disabled(state.preview)
            }
            .padding(.top, 25).padding(.vertical, 14)
        }
        .padding(.horizontal, 28).padding(.top, 31)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onChange(of: state.settings) { state.save() }
        .background(Theme.paper)
    }
}

private struct AppearancePane: View {
    @Bindable var state: AppState
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text("Weergave").font(.figtree(16, weight: .semibold))
            HStack {
                Text("Weergave").font(.figtree(12))
                Spacer()
                PennelPopup(label: "Weergave", options: [(.system, "Volg systeem"), (.light, "Licht"), (.dark, "Donker")], selection: $state.settings.appearance).frame(width: 140)
            }
            .padding(.vertical, 13).overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
            Text("Volg systeem past de lichte of donkere weergave aan met je apparaat.").font(.figtree(11)).foregroundStyle(Theme.muted)
            Button("Introductie opnieuw openen") { state.onReplayOnboarding?() }
                .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
