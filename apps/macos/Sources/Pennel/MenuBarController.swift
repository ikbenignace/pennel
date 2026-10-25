import AppKit
import Sparkle
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

@MainActor
final class MenuBarController: NSObject {
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let state: AppState
    private let windows: WindowPresenter
    private let dictation: DictationController
    private let meeting: MeetingController
    private let calendar: CalendarProvider
    private let hintItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
    private var updater: SPUStandardUpdaterController?

    init(
        state: AppState,
        windows: WindowPresenter,
        dictation: DictationController,
        meeting: MeetingController,
        calendar: CalendarProvider
    ) {
        self.calendar = calendar
        self.state = state
        self.windows = windows
        self.dictation = dictation
        self.meeting = meeting
        super.init()
        if !state.preview { updater = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil) }
        state.onCheckUpdates = { [weak self] in self?.updater?.checkForUpdates(nil) }
        state.onResumeOnboarding = { [weak self, weak state] in
            guard let self, let state else { return }
            let readiness = state.dictationSetup
            state.workspace.onboardingInitialized = false
            state.setOnboarding(step: readiness.configured && readiness.canRecord ? .permissions : readiness.resumeStep, status: .pending)
            self.openOnboarding()
        }
        state.onReplayOnboarding = { [weak self, weak state] in
            state?.workspace.onboardingInitialized = false
            state?.setOnboarding(step: .welcome, status: .pending)
            self?.openOnboarding()
        }
        let menu = NSMenu()
        menu.delegate = self
        statusItem.menu = menu
        updateIcon()
    }

    /// Rebuilt on every open so recording state and recent meetings are current.
    private func rebuild(_ menu: NSMenu) {
        menu.removeAllItems()
        if meeting.isRecording {
            let live = NSMenuItem(title: "● \(meeting.title) · \(Self.clock(meeting.elapsed))", action: #selector(openMeeting), keyEquivalent: "")
            live.target = self
            menu.addItem(live)
            menu.addItem(item("Stop opname", #selector(toggleRecording), key: "R", modifiers: [.command, .shift]))
        } else if meeting.phase == .summarizing {
            menu.addItem(item("Opname afronden…", #selector(openMeeting), key: ""))
        } else {
            menu.addItem(item("Start opname", #selector(toggleRecording), key: "R", modifiers: [.command, .shift]))
        }
        menu.addItem(.separator())
        hintItem.isEnabled = false
        hintItem.title = dictation.isListening
            ? "Dicteren — houd \(state.settings.hotkey.label) ingedrukt"
            : "Dicteren wacht op toegang tot Toegankelijkheid"
        menu.addItem(hintItem)
        let recent = state.engine.meetings().prefix(3)
        if !recent.isEmpty {
            menu.addItem(.separator())
            let label = NSMenuItem(title: "Recent", action: nil, keyEquivalent: "")
            label.isEnabled = false
            menu.addItem(label)
            for info in recent {
                let entry = NSMenuItem(title: info.title, action: #selector(openRecent(_:)), keyEquivalent: "")
                entry.target = self
                entry.representedObject = info.id
                menu.addItem(entry)
            }
        }
        menu.addItem(.separator())
        menu.addItem(item("Open Pennel", #selector(openMain), key: "o"))
        menu.addItem(item("Geschiedenis…", #selector(openHistory), key: ""))
        menu.addItem(item("Instellingen…", #selector(openSettings), key: ","))
        menu.addItem(item("Logboek openen", #selector(openLog), key: ""))
        let updates = NSMenuItem(
            title: "Zoek naar updates…",
            action: #selector(SPUStandardUpdaterController.checkForUpdates(_:)),
            keyEquivalent: ""
        )
        updates.target = updater
        updates.isEnabled = updater != nil
        menu.addItem(updates)
        menu.addItem(.separator())
        menu.addItem(item("Stop Pennel", #selector(quit), key: "q"))
    }

    func updateIcon() {
        let recording = meeting.isRecording
        let image = recording
            ? NSImage(systemSymbolName: "record.circle.fill", accessibilityDescription: "Pennel neemt op")
            : Bundle.main.image(forResource: "MenubarTemplate")
        image?.accessibilityDescription = recording ? "Pennel neemt op" : "Pennel"
        image?.isTemplate = !recording
        statusItem.button?.image = image
        statusItem.button?.contentTintColor = recording ? NSColor(Theme.accent) : nil
    }

    @objc func toggleRecording() {
        if meeting.phase == .summarizing { openMeeting(); return }
        if meeting.isRecording {
            meeting.stop()
        } else {
            meeting.reset()
            meeting.start()
            openMeeting()
        }
        updateIcon()
    }

    @objc private func openLog() {
        Diagnostics.reveal()
    }

    @objc func openMeeting() {
        state.workspace.destination = .meetings
        state.workspace.showingRecording = true
        openWorkspace()
    }

    @objc func openLibrary() {
        state.workspace.showingRecording = false
        openWorkspace(.meetings)
    }

    @objc private func openMain() { openWorkspace() }

    @objc private func openRecent(_ sender: NSMenuItem) {
        if let id = sender.representedObject as? String { state.workspace.selectMeeting(id) }
        openWorkspace(.meetings)
        NotificationCenter.default.post(name: .pennelSelectMeeting, object: sender.representedObject)
    }

    private static func clock(_ seconds: TimeInterval) -> String {
        let total = Int(seconds)
        return String(format: "%02d:%02d", total / 60, total % 60)
    }

    private func item(
        _ title: String,
        _ action: Selector,
        key: String,
        modifiers: NSEvent.ModifierFlags = .command
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key.lowercased())
        item.keyEquivalentModifierMask = modifiers
        item.target = self
        return item
    }

    @objc func openSettings() {
        openWorkspace(.settings)
    }

    @objc func openHistory() {
        openWorkspace(.history)
    }

    func openWorkspace(_ destination: WorkspaceDestination? = nil) {
        if let destination { state.workspace.destination = destination }
        NativeAppearance.apply(state.settings.appearance)
        windows.show(id: "main", title: "Pennel", size: NSSize(width: 1040, height: 740), onHide: { [weak state, weak meeting, weak dictation] in
            state?.flushWorkspaceDrafts()
            meeting?.flushDrafts()
            dictation?.shutdownOwnedSession()
        }) {
            MainWorkspaceView(state: state, meeting: meeting, calendar: calendar, openOnboarding: { [weak self] in self?.openOnboarding() })
        }
    }

    func openOnboarding() {
        if let session = state.workspace.session { dictation.closeSession(session.id) }
        if state.settings.onboarding.status != .pending {
            state.workspace.onboardingInitialized = false
            let readiness = state.dictationSetup
            let step = readiness.canRecord && !readiness.canUseShortcut ? OnboardingStep.permissions : readiness.resumeStep
            state.setOnboarding(step: step, status: .pending)
        }
        NativeAppearance.apply(state.settings.appearance)
        windows.show(id: "onboarding", title: "Welkom bij Pennel", size: NSSize(width: 850, height: 690), style: .onboarding, onHide: { [weak state, weak dictation] in
            guard let state else { return }
            if let session = state.workspace.session, session.kind == .practice { dictation?.closeSession(session.id) }
            if state.settings.onboarding.status != .completed { state.setOnboarding(step: state.settings.onboarding.resumeStep, status: .skipped) }
            state.workspace.initialSetup = false
        }, onResign: { [weak state, weak dictation] in
            if let session = state?.workspace.session, session.kind == .practice { dictation?.closeSession(session.id) }
        }) {
            OnboardingView(state: state, onFinish: { [weak self] in
                self?.windows.hide("onboarding")
                self?.openWorkspace(.dictation)
            })
        }
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}

extension MenuBarController: NSMenuDelegate {
    func menuWillOpen(_ menu: NSMenu) {
        rebuild(menu)
    }
}

extension HotkeyTrigger {
    var label: String {
        switch self {
        case .fn: "fn"
        case .controlSuper: "Ctrl + Super"
        case .rightOption: "rechter Option"
        case .rightCommand: "rechter Command"
        case .rightControl: "rechter Ctrl"
        }
    }
}
