import PennelMobileKit
import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var model
    @Bindable var router: AppRouter

    var body: some View {
        TabView(selection: $router.selectedTab) {
            Tab(Copy.Tab.meetings, systemImage: "waveform", value: AppTab.meetings) {
                MeetingListView()
            }
            Tab(Copy.Tab.dictate, systemImage: "mic", value: AppTab.dictate) {
                DictationView()
            }
            Tab(Copy.Tab.ask, systemImage: "bubble.left.and.text.bubble.right", value: AppTab.ask) {
                AskView()
            }
            Tab(Copy.Tab.settings, systemImage: "gearshape", value: AppTab.settings) {
                SettingsView()
            }
        }
        .font(Theme.font(.body))
        .tint(Brand.accent.color)
        .preferredColorScheme(model.settings.appearance.colorScheme)
        .onAppear {
            if model.settings.onboarding.status == .pending { router.showsOnboarding = true }
        }
        .fullScreenCover(isPresented: $router.showsOnboarding) {
            OnboardingView { status in
                model.update { $0.onboarding.status = status }
                router.showsOnboarding = false
            }
            .environment(model)
            .preferredColorScheme(model.settings.appearance.colorScheme)
        }
        .alert(
            model.errorMessage ?? "",
            isPresented: Binding(get: { model.errorMessage != nil }, set: { if !$0 { model.errorMessage = nil } })
        ) {}
    }
}
