import AppKit
import MusliKit

final class SystemPasteboard: PasteboardLike {
    private let pasteboard = NSPasteboard.general

    var changeCount: Int { pasteboard.changeCount }

    func snapshot() -> [[String: Data]] {
        (pasteboard.pasteboardItems ?? []).map { item in
            Dictionary(uniqueKeysWithValues: item.types.compactMap { type in
                item.data(forType: type).map { (type.rawValue, $0) }
            })
        }
    }

    func restore(_ items: [[String: Data]]) {
        pasteboard.clearContents()
        let restored = items.map { entries in
            let item = NSPasteboardItem()
            for (type, data) in entries { item.setData(data, forType: NSPasteboard.PasteboardType(type)) }
            return item
        }
        if !restored.isEmpty { pasteboard.writeObjects(restored) }
    }

    func setString(_ s: String) {
        pasteboard.clearContents()
        pasteboard.setString(s, forType: .string)
        // Convention honoured by clipboard managers so the dictated text doesn't pollute their history.
        pasteboard.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
    }
}

@MainActor
enum TextInserter {
    private static let pasteboard = SystemPasteboard()
    private static let vKey: CGKeyCode = 9

    static func insert(_ text: String) async {
        let token = PasteboardGuard.begin(pasteboard, text: text)
        let source = CGEventSource(stateID: .combinedSessionState)
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: source, virtualKey: vKey, keyDown: down)
            event?.flags = .maskCommand
            event?.post(tap: .cghidEventTap)
        }
        try? await Task.sleep(for: .milliseconds(250))
        PasteboardGuard.end(pasteboard, token)
    }
}
