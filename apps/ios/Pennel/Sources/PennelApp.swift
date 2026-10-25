import SwiftUI

@main
struct PennelApp: App {
    @State private var model = AppModel.shared

    init() {
        Theme.configureNavigationBars()
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
        }
    }
}
