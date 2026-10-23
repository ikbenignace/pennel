import AppKit
@preconcurrency import MusliCore

@MainActor
final class MenuBarController: NSObject {
    private let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let state: AppState
    private let windows: WindowPresenter
    private let dictation: DictationController
    private let hintItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")

    init(state: AppState, windows: WindowPresenter, dictation: DictationController) {
        self.state = state
        self.windows = windows
        self.dictation = dictation
        super.init()
        statusItem.button?.image = NSImage(systemSymbolName: "waveform", accessibilityDescription: "Musli")
        statusItem.button?.image?.isTemplate = true
        statusItem.menu = buildMenu()
    }

    private func buildMenu() -> NSMenu {
        let menu = NSMenu()
        menu.delegate = self
        hintItem.isEnabled = false
        menu.addItem(hintItem)
        menu.addItem(.separator())
        menu.addItem(item("Instellingen…", #selector(openSettings), key: ","))
        menu.addItem(.separator())
        menu.addItem(item("Stop Musli", #selector(quit), key: "q"))
        return menu
    }

    private func item(_ title: String, _ action: Selector, key: String) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    @objc func openSettings() {
        windows.show(id: "settings", title: "Instellingen", size: NSSize(width: 640, height: 520)) {
            SettingsView(state: state)
        }
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}

extension MenuBarController: NSMenuDelegate {
    func menuWillOpen(_ menu: NSMenu) {
        hintItem.title = dictation.isListening
            ? "Dicteren — houd \(state.settings.hotkey.label) ingedrukt"
            : "Dicteren wacht op toegang tot Toegankelijkheid"
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
