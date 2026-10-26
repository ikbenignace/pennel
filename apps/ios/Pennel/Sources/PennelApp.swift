import SwiftUI

@main
struct PennelApp: App {
    @State private var model = AppModel.shared
    @State private var showsSessionReturn = false

    init() {
        Theme.configureNavigationBars()
        SystemSessionActivity.endLeftovers()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .onOpenURL { url in
                    guard DictationSession.isStartLink(url) else { return }
                    showsSessionReturn = true
                    Task { await DictationSession.shared.start() }
                }
                .sheet(isPresented: $showsSessionReturn) {
                    SessionReturnView(session: DictationSession.shared)
                        .presentationDragIndicator(.visible)
                }
        }
    }
}
