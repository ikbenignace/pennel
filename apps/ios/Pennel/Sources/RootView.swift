import PennelMobileKit
import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        TabView {
            Tab(Copy.Tab.meetings, systemImage: "waveform") {
                MeetingListView()
            }
            Tab(Copy.Tab.dictate, systemImage: "mic") {
                DictationView()
            }
            Tab(Copy.Tab.ask, systemImage: "bubble.left.and.text.bubble.right") {
                AskView()
            }
            Tab(Copy.Tab.settings, systemImage: "gearshape") {
                SettingsView()
            }
        }
        .font(Theme.font(.body))
        .tint(Brand.accent.color)
        .preferredColorScheme(model.settings.appearance.colorScheme)
        .alert(
            model.errorMessage ?? "",
            isPresented: Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.errorMessage = nil } })
        ) {}
    }
}
