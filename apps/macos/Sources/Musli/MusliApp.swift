import AppKit

@main
@MainActor
final class MusliApp: NSObject, NSApplicationDelegate {
    private var state: AppState?
    private var menuBar: MenuBarController?
    private let windows = WindowPresenter()

    static func main() {
        let app = NSApplication.shared
        let delegate = MusliApp()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            let state = try AppState()
            self.state = state
            menuBar = MenuBarController(state: state, windows: windows)
            if state.needsOnboarding {
                windows.show(id: "onboarding", title: "Welkom bij Musli", size: NSSize(width: 520, height: 560)) {
                    OnboardingView(state: state)
                }
            }
        } catch {
            let alert = NSAlert()
            alert.messageText = "Musli kon niet starten"
            alert.informativeText = "\(error)"
            alert.runModal()
            NSApp.terminate(nil)
        }
    }
}
