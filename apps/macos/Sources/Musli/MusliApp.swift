import AppKit

@main
@MainActor
final class MusliApp: NSObject, NSApplicationDelegate {
    private var state: AppState?
    private var menuBar: MenuBarController?
    private var dictation: DictationController?
    private var permissionPoll: Timer?
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
            let dictation = DictationController(state: state)
            self.dictation = dictation
            menuBar = MenuBarController(state: state, windows: windows, dictation: dictation)
            startDictation(dictation)
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

    /// The event tap can only be created once Accessibility is granted, which may happen after launch.
    private func startDictation(_ dictation: DictationController) {
        guard !dictation.start() else { return }
        permissionPoll = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard dictation.start() else { return }
                self?.permissionPoll?.invalidate()
                self?.permissionPoll = nil
            }
        }
    }
}
