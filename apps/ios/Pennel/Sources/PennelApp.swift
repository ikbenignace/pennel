import SwiftUI

@main
struct PennelApp: App {
    @State private var model = AppModel.shared
    @State private var showsSessionReturn = false
    @Environment(\.scenePhase) private var scenePhase

    init() {
        Theme.configureNavigationBars()
        SystemSessionActivity.endLeftovers()
        SystemRecordingActivity.endLeftovers()
        SystemFinishScheduler.register()
        CloudSyncBackground.register()
        CloudSyncRunner.shared.attach(to: MeetingRuntime.shared)
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
                .onChange(of: scenePhase, initial: true) { _, phase in
                    switch phase {
                    case .active: CloudSyncRunner.shared.enterForeground()
                    case .background:
                        CloudSyncRunner.shared.enterBackground()
                        CloudSyncBackground.schedule()
                    default: break
                    }
                }
                .sheet(isPresented: $showsSessionReturn) {
                    SessionReturnView(session: DictationSession.shared)
                        .presentationDragIndicator(.visible)
                }
        }
    }
}
