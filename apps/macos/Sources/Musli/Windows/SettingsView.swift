import MusliKit
import SwiftUI

enum SettingsTab: Hashable {
    case providers, models, dictation, dictionary, styles
}

struct SettingsView: View {
    @Bindable var state: AppState
    @State private var tab: SettingsTab = .providers

    var body: some View {
        VStack(spacing: 0) {
            SegmentTabs(
                tabs: [(.providers, "Providers"), (.models, "Modellen"), (.dictation, "Dicteren"), (.dictionary, "Woordenlijst"), (.styles, "Stijlen")],
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
                    case .dictionary: DictionaryPane(state: state)
                    case .styles: StylesPane(state: state)
                    }
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
            }
            if let error = state.lastError {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent).padding(8)
            }
        }
        .frame(minWidth: 640, minHeight: 480)
        .onChange(of: state.settings) { state.save() }
        .background(Theme.paper)
    }
}
