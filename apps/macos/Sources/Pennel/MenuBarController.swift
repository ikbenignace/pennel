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
    private let updater = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)

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
        menu.addItem(item("Open Pennel", #selector(openLibrary), key: "o"))
        menu.addItem(item("Geschiedenis…", #selector(openHistory), key: ""))
        menu.addItem(item("Instellingen…", #selector(openSettings), key: ","))
        menu.addItem(item("Logboek openen", #selector(openLog), key: ""))
        let updates = NSMenuItem(
            title: "Zoek naar updates…",
            action: #selector(SPUStandardUpdaterController.checkForUpdates(_:)),
            keyEquivalent: ""
        )
        updates.target = updater
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
        windows.show(id: "meeting", title: "Meeting", size: NSSize(width: 680, height: 600)) {
            MeetingWindowView(state: state, meeting: meeting)
        }
    }

    @objc func openLibrary() {
        windows.show(id: "library", title: "Pennel", size: NSSize(width: 980, height: 640)) {
            LibraryView(state: state, meeting: meeting, openRecording: { [weak self] in self?.openMeeting() })
        }
    }

    @objc private func openRecent(_ sender: NSMenuItem) {
        openLibrary()
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
        windows.show(id: "settings", title: "Instellingen", size: NSSize(width: 640, height: 520)) {
            SettingsView(state: state, calendar: calendar)
        }
    }

    @objc func openHistory() {
        windows.show(id: "history", title: "Geschiedenis", size: NSSize(width: 560, height: 520)) {
            HistoryView(state: state)
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
        case .rightOption: "rechter ⌥"
        case .rightCommand: "rechter ⌘"
        }
    }
}
