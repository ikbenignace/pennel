import PennelMobileKit
import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        TabView {
            Tab(Copy.Tab.meetings, systemImage: "waveform") {
                PlaceholderScreen(title: Copy.Tab.meetings)
            }
            Tab(Copy.Tab.dictate, systemImage: "mic") {
                PlaceholderScreen(title: Copy.Tab.dictate)
            }
            Tab(Copy.Tab.ask, systemImage: "bubble.left.and.text.bubble.right") {
                PlaceholderScreen(title: Copy.Tab.ask)
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

/// Empty tab until its feature lands.
private struct PlaceholderScreen: View {
    let title: String

    var body: some View {
        NavigationStack {
            List {}
                .pennelList()
                .navigationTitle(title)
        }
    }
}
