import SwiftUI

@main
struct PennelApp: App {
    @State private var model = AppModel.shared
    @State private var router = AppRouter.shared
    @Environment(\.scenePhase) private var scenePhase

    init() {
        Theme.configureNavigationBars()
        SystemSessionActivity.endLeftovers()
        SystemRecordingActivity.endLeftovers()
        DictationSession.purgeExpiredResult()
        SystemFinishScheduler.register()
        CloudSyncBackground.register()
        CloudSyncRunner.shared.attach(to: MeetingRuntime.shared)
    }

    var body: some Scene {
        WindowGroup {
            RootView(router: router)
                .environment(model)
                .onOpenURL { url in
                    guard DictationSession.isStartLink(url) else { return }
                    router.showsSessionReturn = true
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
                .sheet(isPresented: $router.showsSessionReturn) {
                    SessionReturnView(session: DictationSession.shared)
                        .presentationDragIndicator(.visible)
                }
        }
    }
}
