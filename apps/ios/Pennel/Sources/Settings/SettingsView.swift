import PennelMobileKit
import SwiftUI

/// Same sections and order as the desktop settings tabs in `apps/desktop/src/main.ts`.
enum SettingsSection: CaseIterable, Hashable {
    case providers, models, dictation, meetings, recording, vocabulary, styles, appearance, account

    var title: String {
        switch self {
        case .providers: Copy.Settings.providers
        case .models: Copy.Settings.models
        case .dictation: Copy.Settings.dictation
        case .meetings: Copy.Settings.meetings
        case .recording: Copy.Settings.recording
        case .vocabulary: Copy.Settings.vocabulary
        case .styles: Copy.Settings.styles
        case .appearance: "Weergave"
        case .account: Copy.Settings.account
        }
    }

    var systemImage: String {
        switch self {
        case .providers: "server.rack"
        case .models: "cpu"
        case .dictation: "mic"
        case .meetings: "person.2"
        case .recording: "record.circle"
        case .vocabulary: "character.book.closed"
        case .styles: "textformat"
        case .appearance: "circle.lefthalf.filled"
        case .account: "person.crop.circle"
        }
    }

    @MainActor @ViewBuilder
    var destination: some View {
        switch self {
        case .providers: ProvidersView()
        case .models: ModelsView()
        case .dictation: DictationSettingsView()
        case .meetings: MeetingSettingsView()
        case .recording: RecordingRulesView()
        case .vocabulary: DictionaryView()
        case .styles: StylesView()
        case .appearance: AppearanceView()
        case .account: AccountView()
        }
    }
}

struct SettingsView: View {
    var body: some View {
        NavigationStack {
            List(SettingsSection.allCases, id: \.self) { section in
                NavigationLink(value: section) {
                    Label(section.title, systemImage: section.systemImage)
                }
                .pennelRow()
            }
            .pennelList()
            .navigationTitle(Copy.Tab.settings)
            .navigationDestination(for: SettingsSection.self) { section in
                section.destination
                    .navigationTitle(section.title)
                    .navigationBarTitleDisplayMode(.inline)
            }
            .navigationDestination(for: ProviderRoute.self) { route in
                ProviderDetailView(providerId: route.id)
            }
        }
    }
}

struct AppearanceView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section {
                Picker("Weergave", selection: Binding(
                    get: { model.settings.appearance },
                    set: { value in model.update { $0.appearance = value } }
                )) {
                    Text("Volg systeem").tag(AppearancePreference.system)
                    Text("Licht").tag(AppearancePreference.light)
                    Text("Donker").tag(AppearancePreference.dark)
                }
            } footer: {
                Text("Volg systeem past de lichte of donkere weergave aan met je apparaat.")
            }
            .pennelRow()
        }
        .pennelList()
    }
}
