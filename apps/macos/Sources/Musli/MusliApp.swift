import AppKit

@main
@MainActor
final class MusliApp: NSObject, NSApplicationDelegate {
    private var state: AppState?
    private var menuBar: MenuBarController?
    private var dictation: DictationController?
    private var permissionPoll: Timer?
    private var unloadTimer: Timer?
    private var meeting: MeetingController?
    private var calls: CallCoordinator?
    private var shortcut: ShortcutTap?
    private var shortcutPoll: Timer?
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
            state.onHotkeyChange = { [weak dictation] trigger in dictation?.setTrigger(trigger) }
            let meeting = MeetingController(state: state)
            self.meeting = meeting
            dictation.meeting = meeting
            let calendar = CalendarProvider()
            let menuBar = MenuBarController(
                state: state, windows: windows, dictation: dictation, meeting: meeting, calendar: calendar
            )
            self.menuBar = menuBar
            meeting.onLibraryChanged = { [weak menuBar] in
                menuBar?.updateIcon()
                NotificationCenter.default.post(name: .musliMeetingsChanged, object: nil)
            }
            let calls = CallCoordinator(state: state, meeting: meeting, calendar: calendar)
            self.calls = calls
            calls.refresh()
            let shortcut = ShortcutTap { [weak menuBar] in
                MainActor.assumeIsolated { menuBar?.toggleRecording() }
            }
            self.shortcut = shortcut
            let applyShortcut = { [weak self, weak shortcut] in
                guard let self, let shortcut else { return }
                if state.settings.recordShortcutEnabled {
                    if !shortcut.start() { self.retryShortcutLater(shortcut) }
                } else {
                    shortcut.stop()
                }
            }
            applyShortcut()
            state.onRecordingSettingsChange = { [weak calls] in
                calls?.refresh()
                applyShortcut()
            }
            startDictation(dictation)
            let engine = state.engine
            DispatchQueue.global(qos: .utility).async { _ = engine.pruneMeetingAudio() }
            unloadTimer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { _ in
                DispatchQueue.global(qos: .utility).async { _ = engine.unloadIdleModels() }
            }
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

    /// The swallowing tap needs Accessibility, which may be granted after launch.
    private func retryShortcutLater(_: ShortcutTap) {
        shortcutPoll?.invalidate()
        shortcutPoll = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, self.state?.settings.recordShortcutEnabled == true, self.shortcut?.start() == true else { return }
                self.shortcutPoll?.invalidate()
                self.shortcutPoll = nil
            }
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let meeting, meeting.isRecording else { return .terminateNow }
        Task {
            await meeting.stopForQuit()
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
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
