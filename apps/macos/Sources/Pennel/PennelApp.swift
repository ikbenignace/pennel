import AppKit

@main
@MainActor
final class PennelApp: NSObject, NSApplicationDelegate {
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
        let delegate = PennelApp()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            let state = try AppState()
            self.state = state
            NativeAppearance.apply(state.settings.appearance)
            ExternalPasteCoordinator.shared.startTracking()
            let dictation = DictationController(state: state)
            self.dictation = dictation
            state.dictation = dictation
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
                NotificationCenter.default.post(name: .pennelMeetingsChanged, object: nil)
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
                menuBar.openOnboarding()
            } else { menuBar.openWorkspace() }
        } catch {
            let alert = NSAlert()
            alert.messageText = "Pennel kon niet starten"
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

    /// Pennel has no Dock icon, so opening the app again (Finder, Spotlight, `open`) shows the library.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        menuBar?.openWorkspace()
        return false
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if state?.workspace.templateEditing != nil {
            state?.lastError = "Bewaar of annuleer eerst het geopende sjabloon."
            menuBar?.openWorkspace(.settings)
            return .terminateCancel
        }
        if state?.flushWorkspaceDrafts() == false { menuBar?.openWorkspace(); return .terminateCancel }
        dictation?.shutdownOwnedSession()
        guard let meeting, meeting.isRecording || meeting.phase == .summarizing else { return .terminateNow }
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
