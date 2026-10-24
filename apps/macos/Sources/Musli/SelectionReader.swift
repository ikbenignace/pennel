import AppKit
import ApplicationServices

/// Reads the text selected in the frontmost app: Accessibility first, a guarded ⌘C for apps that don't expose it.
@MainActor
enum SelectionReader {
    private static let cKey: CGKeyCode = 8
    private static let copyWait: Duration = .milliseconds(300)

    static func read() async -> String? {
        switch axSelection() {
        case .some(let text): return text.isEmpty ? nil : text
        case .none: return await copiedSelection()
        }
    }

    /// `""` when the app supports selection but nothing is selected, `nil` when it doesn't say.
    private static func axSelection() -> String? {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.25)
        var focused: CFTypeRef?
        guard AXUIElementCopyAttributeValue(system, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
              let focused, CFGetTypeID(focused) == AXUIElementGetTypeID()
        else { return nil }
        let element = focused as! AXUIElement
        var selected: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXSelectedTextAttribute as CFString, &selected) == .success else {
            return nil
        }
        return selected as? String
    }

    /// Copies with ⌘C and restores the clipboard; an unchanged clipboard means nothing was selected.
    private static func copiedSelection() async -> String? {
        let pasteboard = NSPasteboard.general
        let system = SystemPasteboard()
        let saved = system.snapshot()
        let before = pasteboard.changeCount
        let source = CGEventSource(stateID: .combinedSessionState)
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: source, virtualKey: cKey, keyDown: down)
            event?.flags = .maskCommand
            event?.post(tap: .cghidEventTap)
        }
        let deadline = ContinuousClock.now + copyWait
        while pasteboard.changeCount == before, ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(20))
        }
        guard pasteboard.changeCount != before else { return nil }
        let text = pasteboard.string(forType: .string)
        system.restore(saved)
        return text?.isEmpty == false ? text : nil
    }
}
