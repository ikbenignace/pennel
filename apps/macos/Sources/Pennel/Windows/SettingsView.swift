import PennelKit
import SwiftUI

enum SettingsTab: Hashable {
    case providers, models, dictation, meetings, recording, dictionary, styles, account
}

struct SettingsView: View {
    @Bindable var state: AppState
    let calendar: CalendarProvider
    @State private var tab: SettingsTab = .providers

    var body: some View {
        VStack(spacing: 0) {
            SegmentTabs(
                tabs: [(.providers, "Providers"), (.models, "Modellen"), (.dictation, "Dicteren"), (.meetings, "Meetings"), (.recording, "Opnemen"), (.dictionary, "Woordenlijst"), (.styles, "Stijlen"), (.account, "Account")],
                selection: $tab
            )
                .padding(.top, 14)
                .padding(.bottom, 10)
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
                    case .account: AccountPane()
                    }
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
            }
            if let error = state.lastError {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent).padding(8)
            }
        }
        .frame(minWidth: 760, minHeight: 520)
        .onChange(of: state.settings) { state.save() }
        .background(Theme.paper)
    }
}
